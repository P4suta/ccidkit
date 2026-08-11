# Architecture

Invariants only. Reasoning lives in [`docs/adr/`](docs/adr/).

## Shape

```text
                ccid-cli                    ccid-driverkit
                   ▲                          ▲  ▲  ▲
                ccidkit ──────────────────────┘  │  │   ← facade: Composite, defaults
            ▲      ▲       ▲                     │  │
   backend-usb  backend-pcsc  backend-virtual ───┘  │
      ▲    ▲       ▲    ▲        ▲    ▲             │
      │  ccid-core─┴────┴────────┘    │             │
      │    ▲                          │             │
   ccid-proto ────────────────────────┼─────────────┘
        ▲                             │
     ccid-apdu ◄──────────────────────┘
```

`ccid-cli` names only `ccidkit`; `ccid-driverkit` deliberately reaches below the facade
(core, apdu, proto, the USB and virtual backends) because diagnosing a reader means
seeing the layers the facade hides. `ccid-testkit` is outside the shipped graph
entirely: dev-dependency tables only.

## Invariants

1. **Native is the goal; the shim is the scaffold**
   ([ADR 0001](docs/adr/0001-native-is-the-goal-shim-is-the-scaffold.md)). The Rust
   DevEx API is the product; `ccid-backend-usb` is the measuring stick, and
   `ccid-backend-pcsc` is both verification scaffold and permanent coexistence route.

2. **The protocol core is sans-I/O**
   ([ADR 0002](docs/adr/0002-sans-io-protocol-core.md)). `ccid-proto` holds no I/O, no
   clock, and no USB types. Machines emit actions; transports own time and retries.

3. **Static dispatch; heterogeneity lives in one enum**
   ([ADR 0003](docs/adr/0003-afit-static-dispatch.md)). No `dyn`, no async-trait crate,
   no backend enum in `ccid-core`. The facade's `Composite` is the only runtime switch.

4. **Exclusivity is spoken by types**
   ([ADR 0004](docs/adr/0004-exclusivity-is-spoken-by-types.md)). Every operation takes
   `&mut self`; `Transaction` is a concrete borrow guard over `&mut Card`. Proven by a
   compile-fail doctest pair, not by a runtime lock.

5. **A status word is data, not an error**
   ([ADR 0005](docs/adr/0005-sw-is-data-not-error.md)). `transmit` returns
   `Ok(Response)` for any SW; `Err` means transport or protocol failure.
   `61xx`/`6Cxx`/extended length/chaining are absorbed by `transmit`, exposed raw by
   `transmit_raw`.

6. **Coexist; do not fight the OS**
   ([ADR 0006](docs/adr/0006-coexist-do-not-fight-the-os.md)). Platform defaults:
   Linux native + `pcscd` collision diagnosis; Windows `winscard` shim, WinUSB rebind
   opt-in; macOS `PCSC.framework` shim. No device is ever stolen.

7. **One shim crate covers three platforms**
   ([ADR 0007](docs/adr/0007-one-shim-crate.md)) through the `pcsc` crate; a two-crate
   hand-written FFI split is the recorded fallback.

8. **T machines are enums**
   ([ADR 0008](docs/adr/0008-t-machines-are-enums.md)): `step` functions over data
   states, not typestates. They never appear in a core trait.

9. **Quirks are data with provenance**
   ([ADR 0009](docs/adr/0009-quirks-are-data-with-provenance.md)):
   `quirks/readers.toml`, one reproduced entry at a time, schema enforced by
   `just quirkdb`.

10. **Cassettes are directional and time-free**
    ([ADR 0010](docs/adr/0010-cassettes-are-directional-and-time-free.md)): `PcToRdr`
    and `RdrToPc` explicit, no timestamps, hex serde, one schema with `apdu` and `ccid`
    flavors.

11. **`!Send` is contained in workers; `unsafe` in one crate**
    ([ADR 0011](docs/adr/0011-not-send-is-contained-in-workers.md)). The shim runs one
    worker per context with a `Job` enum, oneshot replies, a priority cancel lane, and
    drop-fired cancel. `ccid-core` never demands `Send`. `unsafe` is confined to
    `crates/ccid-backend-pcsc/src/` by `just unsafe-boundary`.

12. **The error vocabulary is flat**
    ([ADR 0012](docs/adr/0012-flat-error-vocabulary.md)): `non_exhaustive`, no source
    chain, retryable conditions as their own variants, no SW inside.

13. **The zero-dependency charter**
    ([ADR 0013](docs/adr/0013-zero-dep-charter.md)): `ccid-apdu` depends on nothing;
    `ccid-core` and `ccid-proto` on `ccid-apdu` alone; nothing third-party arrives
    transitively in the pure layer. Enforced by `just purity` and `just deps`.

14. **The virtual backend is a product**
    ([ADR 0014](docs/adr/0014-virtual-backend-is-a-product.md)): published, scriptable,
    with `hang()` as the permanent cancellation-safety fixture.

15. **Naming** ([ADR 0015](docs/adr/0015-naming.md)): facade `ccidkit`, lib prefix
    `ccid-*`, binaries `ccid` and `ccdev`, disjoint from lib names by `just bin-name`.

## What is gated

| Gate | Enforces | Runs |
| --- | --- | --- |
| `just purity` | the pure layer takes nothing from outside itself, dev deps included | offline, pre-commit |
| `just deps` | every edge is an arrow the ALLOWED matrix carries (R1, R2, R3, R5, R7) | offline, pre-commit |
| `just unsafe-boundary` | `unsafe` only in the shim, each block under `// SAFETY:` | offline, pre-commit |
| `just quirkdb` | quirk table sorted, unique, attributed, in vocabulary | offline, pre-commit |
| `just bin-name` | no bin target shares a lib crate's name | offline |
| `just lint` / `just test` | the workspace lint wall and the suite | offline, pre-commit / pre-push |

Invariants 2, 4, 5, and 11's worker model have no standalone gate. They rest on API
design, the compile-fail doctest pair (ADR 0004), review, and the virtual backend's
scenarios. Every gate that exists was written before the code it governs.

## Crate boundaries

The ALLOWED matrix in [`xtask/src/deps.rs`](xtask/src/deps.rs) is the machine-checked
form of this table (transitively closed, self-tested). Direct intent:

| Crate | Directly depends on | May also reach (closure) |
| --- | --- | --- |
| `ccid-apdu` | — | — |
| `ccid-core` | `ccid-apdu` | — |
| `ccid-proto` | `ccid-apdu` | — |
| `ccid-testkit` | — (dev-only, reached from dev tables alone) | — |
| `ccid-backend-usb` | `ccid-core`, `ccid-apdu`, `ccid-proto` | — |
| `ccid-backend-pcsc` | `ccid-core`, `ccid-apdu` | — |
| `ccid-backend-virtual` | `ccid-core`, `ccid-apdu` | — |
| `ccidkit` | `ccid-core`, `ccid-apdu`, the three backends | `ccid-proto` |
| `ccid-cli` | `ccidkit` | everything the facade reaches |
| `ccid-driverkit` | `ccid-core`, `ccid-apdu`, `ccid-proto`, `ccid-backend-usb`, `ccid-backend-virtual` | — |

No inter-crate dependency is declared until code needs it; the intent above and the
matrix carry the graph until then, so `cargo shear` stays meaningful.

## What lives where

- **Vocabulary** (`Command`, `Sw`, `Response`, `Atr`, `Aid`) lives in `ccid-apdu` and
  nowhere else; no layer re-invents a status word.
- **Policy-free protocol** lives in `ccid-proto`: what bytes mean, never when to send
  them. The `Exchanger` plans per exchange level — APDU-level readers pass through,
  TPDU-level readers run the T machine, character-level readers are a stated day-one
  error.
- **Time, retries, and transfers** live in the backends. The USB backend applies the
  quirk table at open; quirks are never inferred mid-conversation.
- **Composition** — choosing a backend, platform defaults, the quickstart — lives in
  `ccidkit`. The `Composite` enum delegates by hand-written `match`.
- **Diagnosis** lives in `ccid-driverkit`, which is allowed to see below the facade
  precisely because it is `publish = false`.
