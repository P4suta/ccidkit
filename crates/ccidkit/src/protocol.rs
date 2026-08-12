// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pure ISO/IEC 7816 T=1 block codec and conversation machine.

use crate::{Error, ErrorKind, Result};

const PROLOGUE_LENGTH: usize = 3;
const EPILOGUE_LENGTH: usize = 1;
const MAX_INFORMATION_LENGTH: usize = 254;
const S_WTX: u8 = 0x03;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Block {
    I {
        sequence: bool,
        more: bool,
        information: Box<[u8]>,
    },
    R {
        sequence: bool,
        error: u8,
    },
    S {
        response: bool,
        kind: u8,
        information: Box<[u8]>,
    },
}

impl Block {
    fn encode(&self) -> Result<Vec<u8>> {
        let (pcb, information) = match self {
            Self::I {
                sequence,
                more,
                information,
            } => {
                let mut pcb = u8::from(*sequence) << 6;
                pcb |= u8::from(*more) << 5;
                (pcb, information.as_ref())
            },
            Self::R { sequence, error } => {
                if *error > 0x03 {
                    return Err(protocol_error("T=1 R-block error code is reserved"));
                }
                let pcb = 0x80_u8
                    .checked_add(u8::from(*sequence) << 4)
                    .and_then(|value| value.checked_add(*error))
                    .ok_or_else(|| protocol_error("T=1 R-block PCB overflows"))?;
                (pcb, &[][..])
            },
            Self::S {
                response,
                kind,
                information,
            } => {
                if *kind > 0x1F {
                    return Err(protocol_error("T=1 S-block type is reserved"));
                }
                let pcb = 0xC0_u8
                    .checked_add(u8::from(*response) << 5)
                    .and_then(|value| value.checked_add(*kind))
                    .ok_or_else(|| protocol_error("T=1 S-block PCB overflows"))?;
                (pcb, information.as_ref())
            },
        };
        if information.len() > MAX_INFORMATION_LENGTH {
            return Err(protocol_error("T=1 information field exceeds 254 bytes"));
        }
        let length = u8::try_from(information.len())
            .map_err(|_| protocol_error("T=1 information length does not fit"))?;
        let mut encoded = Vec::with_capacity(
            PROLOGUE_LENGTH
                .saturating_add(information.len())
                .saturating_add(EPILOGUE_LENGTH),
        );
        encoded.extend_from_slice(&[0, pcb, length]);
        encoded.extend_from_slice(information);
        encoded.push(lrc(&encoded));
        Ok(encoded)
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        let nad = bytes
            .first()
            .copied()
            .ok_or_else(|| protocol_error("T=1 block is truncated"))?;
        if nad != 0 {
            return Err(protocol_error("T=1 non-zero NAD is not negotiated"));
        }
        let pcb = bytes
            .get(1)
            .copied()
            .ok_or_else(|| protocol_error("T=1 PCB is truncated"))?;
        let length = usize::from(
            bytes
                .get(2)
                .copied()
                .ok_or_else(|| protocol_error("T=1 LEN is truncated"))?,
        );
        let expected = PROLOGUE_LENGTH
            .checked_add(length)
            .and_then(|value| value.checked_add(EPILOGUE_LENGTH))
            .ok_or_else(|| protocol_error("T=1 block length overflows"))?;
        if bytes.len() != expected || lrc(bytes) != 0 {
            return Err(protocol_error("T=1 block length or LRC is invalid"));
        }
        let information = bytes
            .get(PROLOGUE_LENGTH..PROLOGUE_LENGTH.saturating_add(length))
            .ok_or_else(|| protocol_error("T=1 information field is truncated"))?
            .to_vec()
            .into_boxed_slice();
        if pcb & 0x80 == 0 {
            Ok(Self::I {
                sequence: pcb & 0x40 != 0,
                more: pcb & 0x20 != 0,
                information,
            })
        } else if pcb & 0xC0 == 0x80 {
            if !information.is_empty() || pcb & 0x2C != 0 {
                return Err(protocol_error("T=1 R-block contains reserved fields"));
            }
            Ok(Self::R {
                sequence: pcb & 0x10 != 0,
                error: pcb & 0x03,
            })
        } else {
            Ok(Self::S {
                response: pcb & 0x20 != 0,
                kind: pcb & 0x1F,
                information,
            })
        }
    }
}

/// The next transport-independent action in a T=1 APDU exchange.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum T1Action {
    Send(Box<[u8]>),
    Complete(Box<[u8]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Sending,
    Receiving,
    Complete,
}

/// One APDU conversation over T=1 with LRC error detection.
#[derive(Debug)]
pub(crate) struct T1Machine {
    command: Box<[u8]>,
    offset: usize,
    ifsc: usize,
    send_sequence: bool,
    receive_sequence: bool,
    phase: Phase,
    last_sent: Box<[u8]>,
    response: Vec<u8>,
}

impl T1Machine {
    pub(crate) fn new(command: Vec<u8>, ifsc: u8) -> Result<Self> {
        if command.is_empty() {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "T=1 cannot exchange an empty APDU",
            ));
        }
        if ifsc == 0 || usize::from(ifsc) > MAX_INFORMATION_LENGTH {
            return Err(protocol_error("T=1 IFSC must be in 1..=254"));
        }
        Ok(Self {
            command: command.into_boxed_slice(),
            offset: 0,
            ifsc: usize::from(ifsc),
            send_sequence: false,
            receive_sequence: false,
            phase: Phase::Sending,
            last_sent: Box::new([]),
            response: Vec::new(),
        })
    }

    pub(crate) fn start(&mut self) -> Result<T1Action> {
        if self.offset != 0 || self.phase != Phase::Sending {
            return Err(protocol_error("T=1 conversation was already started"));
        }
        self.send_next_command_block()
    }

    pub(crate) fn accept(&mut self, bytes: &[u8]) -> Result<T1Action> {
        if self.phase == Phase::Complete {
            return Err(protocol_error("T=1 conversation is already complete"));
        }
        let block = Block::decode(bytes)?;
        if let Block::S {
            response: false,
            kind: S_WTX,
            information,
        } = &block
        {
            if information.len() != 1 || information.first().copied() == Some(0) {
                return Err(protocol_error("T=1 WTX request has an invalid multiplier"));
            }
            let response = Block::S {
                response: true,
                kind: S_WTX,
                information: information.clone(),
            }
            .encode()?
            .into_boxed_slice();
            return Ok(T1Action::Send(response));
        }

        match (self.phase, block) {
            (Phase::Sending, Block::R { sequence, error: 0 })
                if self.offset < self.command.len() && sequence != self.send_sequence =>
            {
                self.send_sequence = !self.send_sequence;
                self.send_next_command_block()
            },
            (Phase::Sending, Block::R { sequence, .. }) if sequence == self.send_sequence => {
                Ok(T1Action::Send(self.last_sent.clone()))
            },
            (Phase::Sending, Block::I { .. }) if self.offset < self.command.len() => Err(
                protocol_error("card responded before all chained T=1 command blocks were sent"),
            ),
            (
                Phase::Sending | Phase::Receiving,
                Block::I {
                    sequence,
                    more,
                    information,
                },
            ) => {
                if sequence != self.receive_sequence {
                    return self.send(&Block::R {
                        sequence: self.receive_sequence,
                        error: 0,
                    });
                }
                self.phase = Phase::Receiving;
                self.response.extend_from_slice(&information);
                if more {
                    self.receive_sequence = !self.receive_sequence;
                    self.send(&Block::R {
                        sequence: self.receive_sequence,
                        error: 0,
                    })
                } else {
                    self.phase = Phase::Complete;
                    Ok(T1Action::Complete(
                        std::mem::take(&mut self.response).into_boxed_slice(),
                    ))
                }
            },
            _ => Err(protocol_error(
                "unexpected T=1 block for conversation state",
            )),
        }
    }

    fn send_next_command_block(&mut self) -> Result<T1Action> {
        let end = self
            .offset
            .checked_add(self.ifsc)
            .map_or(self.command.len(), |value| value.min(self.command.len()));
        let information = self
            .command
            .get(self.offset..end)
            .ok_or_else(|| protocol_error("T=1 command offset is invalid"))?
            .to_vec()
            .into_boxed_slice();
        self.offset = end;
        self.send(&Block::I {
            sequence: self.send_sequence,
            more: self.offset < self.command.len(),
            information,
        })
    }

    fn send(&mut self, block: &Block) -> Result<T1Action> {
        let encoded = block.encode()?.into_boxed_slice();
        self.last_sent.clone_from(&encoded);
        Ok(T1Action::Send(encoded))
    }
}

fn lrc(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |accumulator, byte| accumulator ^ byte)
}

fn protocol_error(message: &'static str) -> Error {
    Error::new(ErrorKind::Protocol, message)
}

#[cfg(test)]
mod tests {
    use super::{Block, T1Action, T1Machine};

    fn encoded(block: &Block) -> Vec<u8> {
        block.encode().expect("encode")
    }

    #[test]
    fn block_codec_round_trips_every_family_and_rejects_bad_lrc() {
        let blocks = [
            Block::I {
                sequence: true,
                more: true,
                information: Box::from([1, 2]),
            },
            Block::R {
                sequence: true,
                error: 1,
            },
            Block::S {
                response: false,
                kind: 3,
                information: Box::from([2]),
            },
        ];
        for block in blocks {
            let bytes = encoded(&block);
            assert!(matches!(Block::decode(&bytes), Ok(decoded) if decoded == block));
            let mut corrupted = bytes;
            if let Some(last) = corrupted.last_mut() {
                *last ^= 1;
            }
            assert!(Block::decode(&corrupted).is_err());
        }
    }

    #[test]
    fn conversation_chains_command_and_response() {
        let mut machine = T1Machine::new(vec![1, 2, 3], 2).expect("machine");
        assert!(
            matches!(machine.start(), Ok(T1Action::Send(ref bytes)) if matches!(Block::decode(bytes), Ok(decoded) if decoded == Block::I {
                sequence: false,
                more: true,
                information: Box::from([1, 2]),
            }))
        );
        let ack = encoded(&Block::R {
            sequence: true,
            error: 0,
        });
        assert!(
            matches!(machine.accept(&ack), Ok(T1Action::Send(ref bytes)) if matches!(Block::decode(bytes), Ok(decoded) if decoded == Block::I {
                sequence: true,
                more: false,
                information: Box::from([3]),
            }))
        );
        let first = encoded(&Block::I {
            sequence: false,
            more: true,
            information: Box::from([0x90]),
        });
        assert!(matches!(machine.accept(&first), Ok(T1Action::Send(_))));
        let last = encoded(&Block::I {
            sequence: true,
            more: false,
            information: Box::from([0]),
        });
        assert!(matches!(
            machine.accept(&last),
            Ok(T1Action::Complete(bytes)) if bytes.as_ref() == [0x90, 0]
        ));
    }

    #[test]
    fn conversation_answers_wtx_without_advancing() {
        let mut machine = T1Machine::new(vec![0], 32).expect("machine");
        let first = machine.start().expect("start");
        let request = encoded(&Block::S {
            response: false,
            kind: 3,
            information: Box::from([4]),
        });
        let reply = machine.accept(&request).expect("WTX");
        assert_ne!(reply, first);
        assert!(
            matches!(reply, T1Action::Send(ref bytes) if matches!(Block::decode(bytes), Ok(decoded) if decoded == Block::S {
                response: true,
                kind: 3,
                information: Box::from([4]),
            }))
        );
    }

    #[test]
    fn block_boundaries_and_reserved_r_fields_are_enforced() {
        for error in [0, 3] {
            assert!(
                Block::R {
                    sequence: false,
                    error,
                }
                .encode()
                .is_ok()
            );
        }
        assert!(
            Block::R {
                sequence: false,
                error: 4,
            }
            .encode()
            .is_err()
        );
        assert!(
            Block::S {
                response: false,
                kind: 31,
                information: Box::new([]),
            }
            .encode()
            .is_ok()
        );
        assert!(
            Block::S {
                response: false,
                kind: 32,
                information: Box::new([]),
            }
            .encode()
            .is_err()
        );
        assert!(
            Block::I {
                sequence: false,
                more: false,
                information: vec![0; 254].into_boxed_slice(),
            }
            .encode()
            .is_ok()
        );
        assert!(
            Block::I {
                sequence: false,
                more: false,
                information: vec![0; 255].into_boxed_slice(),
            }
            .encode()
            .is_err()
        );

        assert!(matches!(
            Block::decode(&encoded(&Block::R {
                sequence: false,
                error: 0,
            })),
            Ok(Block::R {
                sequence: false,
                error: 0
            })
        ));
        assert!(Block::decode(&[0, 0x80, 1, 0, 0x81]).is_err());
        assert!(Block::decode(&[0, 0x84, 0, 0x84]).is_err());
    }

    #[test]
    fn conversation_rejects_restart_and_bad_wtx_forms() {
        let mut machine = T1Machine::new(vec![0], 32).expect("machine");
        let _first = machine.start().expect("start");
        assert!(machine.start().is_err());
        for information in [Vec::new(), vec![0], vec![1, 2]] {
            let request = encoded(&Block::S {
                response: false,
                kind: 3,
                information: information.into_boxed_slice(),
            });
            assert!(machine.accept(&request).is_err());
        }
    }

    #[test]
    fn conversation_distinguishes_ack_retry_and_premature_response() {
        let mut machine = T1Machine::new(vec![1, 2, 3], 2).expect("machine");
        let first = machine.start().expect("start");
        let retry = encoded(&Block::R {
            sequence: false,
            error: 1,
        });
        assert_eq!(machine.accept(&retry).expect("retry"), first);

        let premature = encoded(&Block::I {
            sequence: false,
            more: false,
            information: Box::from([0x90, 0]),
        });
        assert!(machine.accept(&premature).is_err());

        let ack = encoded(&Block::R {
            sequence: true,
            error: 0,
        });
        let _last_command = machine.accept(&ack).expect("ack");
        let impossible_final_ack = encoded(&Block::R {
            sequence: false,
            error: 0,
        });
        assert!(machine.accept(&impossible_final_ack).is_err());
    }

    #[test]
    fn duplicate_response_block_requests_the_expected_sequence() {
        let mut machine = T1Machine::new(vec![1], 32).expect("machine");
        let _command = machine.start().expect("start");
        let duplicate = encoded(&Block::I {
            sequence: true,
            more: false,
            information: Box::from([9]),
        });
        let action = machine.accept(&duplicate).expect("R-block");
        assert!(
            matches!(action, T1Action::Send(bytes) if matches!(Block::decode(&bytes), Ok(Block::R { sequence: false, error: 0 })))
        );
    }
}
