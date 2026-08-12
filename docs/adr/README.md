# Architecture Decision Records

One record per architectural decision. Each states the context, the decision, and its
consequences as they stand now; superseding a decision adds a new record and marks the
old one `Superseded`. `ARCHITECTURE.md` links here from its invariants list.

Format: numbered file, a `Status` (`Proposed` / `Accepted` / `Superseded`), and a date.
Every decision names the gate that enforces it, because the gate is mechanical, not
cultural — and every gate here was written before the code it governs.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-native-is-the-goal-shim-is-the-scaffold.md) | Native CCID is the goal; the PC/SC shim is scaffold and coexistence route | Superseded by 0016 |
| [0002](0002-sans-io-protocol-core.md) | The protocol core is sans-I/O: no clock or USB types | Superseded by 0016 |
| [0003](0003-afit-static-dispatch.md) | AFIT + static dispatch; heterogeneity only in the facade's `Composite` | Superseded by 0016 |
| [0004](0004-exclusivity-is-spoken-by-types.md) | Exclusivity by `&mut` and a concrete `Transaction` borrow guard | Accepted |
| [0005](0005-sw-is-data-not-error.md) | A status word is data; `Err` means transport or protocol failure | Accepted |
| [0006](0006-coexist-do-not-fight-the-os.md) | Platform default backends; never steal a device the OS holds | Accepted |
| [0007](0007-one-shim-crate.md) | One shim crate over the `pcsc` crate covers three platforms | Superseded by 0016 |
| [0008](0008-t-machines-are-enums.md) | T=0/T=1 are enum state machines with `step`, not typestates | Superseded by 0016 |
| [0009](0009-quirks-are-data-with-provenance.md) | Quirks live in `readers.toml`, one reproduced entry at a time | Accepted |
| [0010](0010-cassettes-are-directional-and-time-free.md) | Cassettes: directional, time-free, hex serde, two flavors one schema | Superseded by 0016 |
| [0011](0011-not-send-is-contained-in-workers.md) | `!Send` contained in per-context workers; `unsafe` in one crate | Superseded by 0016 |
| [0012](0012-flat-error-vocabulary.md) | Flat `non_exhaustive` error vocabulary, retryable variants distinct | Superseded by 0016 |
| [0013](0013-zero-dep-charter.md) | The zero-dependency charter and the ALLOWED matrix | Superseded by 0016 |
| [0014](0014-virtual-backend-is-a-product.md) | The virtual backend is published, `hang()` included | Superseded by 0016 |
| [0015](0015-naming.md) | Naming: facade `ccidkit`, libs `ccid-*`, binaries `ccid` and `ccdev` | Superseded by 0016 |
| [0016](0016-one-library-one-operation.md) | One published library, one runtime-neutral operation, private implementation architecture | Accepted |
