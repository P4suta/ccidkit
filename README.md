# ccidkit

`ccidkit` is a pure-Rust smart-card reader stack with one public library, one I/O
shape, and no runtime allegiance. It speaks native USB CCID on Linux (and opt-in
WinUSB on Windows), uses the platform PC/SC service on Windows and macOS, and ships a
deterministic virtual reader for application tests.

```rust,no_run
let mut card = ccidkit::open_first().wait()?;
let command = ccidkit::Command::new(0x00, 0x84, 0x00, 0x00)
    .with_expected_len(8)?;
let response = card.transmit(command).wait()?;
# Ok::<(), ccidkit::Error>(())
```

The same operation can be awaited:

```rust,no_run
# async fn example() -> ccidkit::Result<()> {
let mut card = ccidkit::open_first().await?;
let response = card
    .transmit(ccidkit::Command::new(0, 0x84, 0, 0))
    .await?;
println!("{}", response.status());
# Ok(())
# }
```

## Design promises

- **One dependency and one semver surface.** `ccidkit` is the only published library.
  APDU, ATR, CCID, T=1, backends, and workers are internal modules rather than future
  compatibility obligations.
- **Runtime-neutral I/O.** Every I/O call returns `Operation<T>`; callers choose
  `.wait()`, `.wait_timeout()`, or `.await` without enabling a Tokio/async-std feature.
- **Exclusivity in the types.** Card operations require `&mut Card`; `Transaction`
  borrows it and maps to a real PC/SC transaction bracket.
- **Wire policy is explicit.** `transmit` absorbs `6Cxx` correction and `61xx` GET
  RESPONSE chaining. `transmit_raw` performs exactly one exchange. A card status word
  is data, never a transport error.
- **No backend SPI.** Built-in backends are selected with a small enum. Backend handles,
  USB descriptors, PC/SC values, and third-party errors never enter the public API.
- **No unsafe code in this repository.** Native and PC/SC access are delegated to
  audited dependencies behind safe APIs.

The full boundary and concurrency model is in [ARCHITECTURE.md](ARCHITECTURE.md). The
decision that consolidated the original prototype is
[ADR-0016](docs/adr/0016-one-library-one-operation.md).

## Backends

| Platform | Default | Optional |
| --- | --- | --- |
| Linux | native USB CCID | PC/SC feature `pcsc` |
| Windows | system PC/SC | WinUSB feature `native-usb` |
| macOS | system PC/SC | — |

Native CCID validates class descriptors and every bulk response, powers and resets
cards, supports APDU-level readers, and drives negotiated TPDU-level T=1/LRC readers through a
sans-I/O block state machine. Character-level readers are rejected explicitly.

## Deterministic tests

```rust
use ccidkit::{Atr, Command, Response, StatusWord};
use ccidkit::testing::Scenario;

let command = Command::new(0, 0x84, 0, 0);
let scenario = Scenario::new()
    .insert(Atr::parse(&[0x3B, 0x00])?)
    .respond(
        command.clone(),
        Response::new([1, 2, 3], StatusWord::from_u16(0x9000)),
    );
let context = ccidkit::testing::open(scenario).wait()?;
let mut card = context.open_first().wait()?;
assert_eq!(card.transmit(command).wait()?.data(), [1, 2, 3]);
# Ok::<(), ccidkit::Error>(())
```

Scenarios can insert, remove, fail, and hang. The permanent `hang()` fixture proves
that dropping an operation releases the ordered card worker without hardware.

## Commands

```text
ccid list
ccid atr
ccid apdu [--raw] 00A4040000
ccdev doctor [native|pcsc]
```

`ccid` depends only on the public library, so the CLI continuously proves that the
facade is sufficient. `ccdev` is private maintainer tooling and deliberately uses the
same stable diagnostic values.

## Development

```sh
just test       # unit, integration, and compile-fail tests
just lint       # all targets/features, warnings denied
just mutants    # targeted semantic mutation testing
just check      # deterministic inner-loop gates
```

Mutation testing targets the parsers and protocol state transitions where a green
example suite can otherwise hide inverted comparisons or skipped validation. Hardware
adapters are excluded because their useful mutations require hardware-in-the-loop.

## License

Dual-licensed under [MIT](LICENSES/MIT.txt) or [Apache-2.0](LICENSES/Apache-2.0.txt), at
your option.
