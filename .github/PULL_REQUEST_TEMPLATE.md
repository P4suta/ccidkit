<!--
SPDX-FileCopyrightText: 2026 ccidkit contributors
SPDX-License-Identifier: MIT OR Apache-2.0
-->

## Summary

<!-- What changes for a user, and why is this the smallest coherent change? -->

## Compatibility budget

<!-- Check each applicable item; explain N/A instead of checking blindly. -->

- [ ] Any public API addition is necessary, minimal, and called out explicitly
- [ ] Backend/library implementation types remain private to `ccidkit`
- [ ] `transmit` still treats every status word as response data; policy begins at
      `require_success`
- [ ] No new runtime-specific public API or third-party backend SPI was introduced

## Protocol and safety

- [ ] Device-derived lengths are checked before allocation, slicing, or loop bounds
- [ ] Arithmetic on device-derived values has explicit overflow behavior
- [ ] Repository Rust remains free of `unsafe` (`just unsafe-boundary`)
- [ ] Parser/state-machine behavior changed here is covered by focused tests and, where
      meaningful, targeted mutation testing

## Hardware and quirks

- [ ] Any new `crates/ccidkit/quirks/readers.toml` entry has our own issue, capture, or
      cassette receipt
- [ ] Any new quirk flag is implemented and added to the closed xtask vocabulary in the
      same change
- [ ] Hardware-dependent claims identify the tested reader, backend, and platform

## Verification

- [ ] `just check` passes locally
- [ ] Relevant full tests (`just test` or `just ci`) pass
- [ ] No local `allow`, ignored test, or weakened gate was added merely to turn CI green

<!-- If a gate changed rather than being satisfied, explain why in the PR body. -->
