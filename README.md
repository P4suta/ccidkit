# ccidkit

A pure-Rust smart card stack: CCID readers over USB, with a PC/SC shim and a scripted
virtual backend.

> **Status: early development.** The workspace, the frozen architecture, and the quality
> gates are in place. **No reader is spoken to yet.** The roadmap and the measurements
> the design rests on are in [ROADMAP.md](ROADMAP.md) and
> [docs/M0-ground-truth.md](docs/M0-ground-truth.md).

## What this is

Talking to a smart card today means going through the platform PC/SC stack — `pcscd`
plus the libccid driver on Linux, `winscard` on Windows, `PCSC.framework` on macOS — a
C-shaped detour with a daemon in the middle. This project's bet is that the CCID driver
layer itself is small enough to own in Rust: the entire per-reader hack logic in libccid
measures 580 lines of C across 43 reader-specific cases, and the rest of its reader
knowledge is declarative data (652 entries, 700 descriptor dumps) that belongs in a
table, not in code (measured in [docs/M0-ground-truth.md](docs/M0-ground-truth.md)).

Two commitments shape everything else:

- **Native is the goal, the shim is the scaffold**
  ([ADR 0001](docs/adr/0001-native-is-the-goal-shim-is-the-scaffold.md)). The product is
  a Rust-native developer experience over USB CCID via `nusb`. The PC/SC shim exists to
  verify the upper layers against real cards before the native backend lands, and it
  stays forever as the coexistence route on platforms whose OS will not hand over the
  device.
- **Coexist; do not fight the OS**
  ([ADR 0006](docs/adr/0006-coexist-do-not-fight-the-os.md)). Defaults per platform:
  Linux native with a named `pcscd`-collision diagnosis, Windows over the `winscard`
  shim with WinUSB rebinding as an explicit opt-in, macOS over the `PCSC.framework`
  shim. Nothing here ever steals a device an OS service holds.

PC/SC C API compatibility is a non-goal. So are EMV payment stacks and card OS
implementations — this is the reader layer, done well.

## Crates

| Crate | Responsibility | Published |
| --- | --- | --- |
| `ccid-apdu` | APDU/SW/ATR/AID vocabulary. Zero dependencies, forever | yes |
| `ccid-core` | `Backend`/`Reader`/`Card`/`Transaction`/`Monitor` traits + errors | yes |
| `ccid-proto` | Sans-I/O CCID codec, descriptor interpretation, T=0/T=1 machines | yes |
| `ccid-testkit` | Dev-only test helpers | no |
| `ccid-backend-usb` | Native CCID over `nusb`: the destination | yes |
| `ccid-backend-pcsc` | The PC/SC shim: scaffold and coexistence route | yes |
| `ccid-backend-virtual` | Scripted reader and card, shipped for downstream tests | yes |
| `ccidkit` | The facade: re-exports, `Composite`, platform defaults, quickstart | yes |
| `ccid-cli` | The `ccid` command | yes |
| `ccid-driverkit` | The `ccdev` bring-up and diagnosis tool | no |

The facade is `ccidkit`; the binaries are `ccid` and `ccdev`. A binary never shares a
lib crate's name ([ADR 0015](docs/adr/0015-naming.md), enforced by `just bin-name`).
Full shape and boundaries: [ARCHITECTURE.md](ARCHITECTURE.md) and the
[decision records](docs/adr/).

## The virtual backend is a product

`ccid-backend-virtual` is published, not a dev fixture
([ADR 0014](docs/adr/0014-virtual-backend-is-a-product.md)): a downstream PIV or eID
implementation needs a reader that inserts, removes, answers, misbehaves, and hangs on
schedule, exactly as this workspace's own CI does. `Scenario` is a consuming builder and
`hang()` is a permanent fixture, because `wait_for_card` cancellation safety is proven,
not promised.

## Quirks are data with provenance

Reader misbehavior lives in [`quirks/readers.toml`](quirks/readers.toml): one entry per
model, each carrying the reproduction that earned it — a cassette, an issue, or a
capture ([ADR 0009](docs/adr/0009-quirks-are-data-with-provenance.md)). libccid's
Info.plist may be consulted to confirm a suspicion, never bulk-transcribed. `just
quirkdb` enforces the schema mechanically, starting from today's zero entries.

## Development

```sh
mise install        # toolchain and gate tooling
mise run hooks      # install the git hooks
just                # list the available commands
just check          # fast inner-loop gates
just ci             # every gate that runs offline
```

`just ci` runs offline and predicts the CI pipeline. The workspace-specific gates —
`purity`, `deps`, `unsafe-boundary`, `quirkdb`, `bin-name` — are implemented in
[`xtask/`](xtask/) with no dependencies of their own, and every one of them was written
before the code it governs exists.

## License

Dual-licensed under [MIT](LICENSES/MIT.txt) or [Apache-2.0](LICENSES/Apache-2.0.txt), at
your option. The repository is [REUSE](https://reuse.software/)-compliant.
