# Contributing

## Setup

```sh
mise install        # toolchain and gate tooling
mise run hooks      # install the git hooks
just                # list the available commands
```

## The loop

```sh
just check          # fast deterministic gates
just ci             # every gate that runs offline
```

`just ci` is also the pre-push hook. If it passes locally it passes in CI. If it fails,
fix the cause rather than narrowing the gate.

## Rules that are not negotiable

- **No `allow`, no `ignore`, no gate suppression.** Do not write `#[allow(...)]`, do not
  add a word to the `typos` allowlist to make a spelling pass, and do not exclude a file
  from a check. Every gate is strict on purpose, and a suppression is invisible to the
  next reader in a way a failing build is not.

  The legitimate escape hatch is the **shared** configuration: `clippy.toml`, the
  `[workspace.lints]` table in `Cargo.toml`, `deny.toml`. If a lint is genuinely wrong
  for this codebase, change it there for the whole workspace, say why in the commit
  message, and leave the reason in a comment next to the setting. That turns one
  person's local exception into a decision the repository made, which is the entire
  difference.

  One setting to know about: **`doc-valid-idents` in `clippy.toml` must keep `".."` as
  its last entry** — without it the list replaces Clippy's defaults instead of extending
  them.

- **A status word never becomes an `Err`.** `transmit` returns `Ok(Response)` for any
  SW; only the caller's `require_success` decides that `6982` is a failure for its purposes.
  If you find yourself matching an SW to return an error from a backend, the layering
  has broken ([ADR 0005](docs/adr/0005-sw-is-data-not-error.md)).

- **The crate graph is the ALLOWED matrix.** `just deps` checks every edge against
  `xtask/src/deps.rs`, which is transitively closed and self-tested. A new crate needs a
  row, and the row is a design decision, not a formality
  ([ADR 0016](docs/adr/0016-one-library-one-operation.md)).

- **Pure protocol code has no effects.** `just purity` scans the model, CCID, and T=1
  modules and rejects I/O, time, worker, and backend imports. Keep property tests and
  byte fixtures in those modules if they remain dependency-free; drive effects through
  the virtual backend.

- **Repository code is safe Rust.** `just unsafe-boundary` rejects unsafe blocks and
  unsafe functions everywhere. FFI invariants belong to the safe adapter dependencies,
  not a local quarantine ([ADR 0016](docs/adr/0016-one-library-one-operation.md)).

- **A quirk entry needs a receipt.** Every `crates/ccidkit/quirks/readers.toml` entry carries a
  reproduction of our own — a cassette, an issue, or a capture. libccid's Info.plist is
  a place to confirm a suspicion, never a source to transcribe: their entries are
  observations about their code paths, and copying one imports a conclusion without its
  evidence ([ADR 0009](docs/adr/0009-quirks-are-data-with-provenance.md)). `just
  quirkdb` enforces the schema; the receipt is on you.

- **Nothing is sized or looped from a number in a device's answer** without checking it
  against the received byte count first. Readers lie; that is why the quirk table
  exists. State overflow behavior in the code rather than inheriting it from the release
  profile.

## Licensing every file you add

The repository is [REUSE](https://reuse.software/)-compliant and `just reuse` enforces
it. **Every source and config file you add opens with the same three lines**, in
whatever comment syntax the format uses:

```rust
// SPDX-FileCopyrightText: 2026 ccidkit contributors
//
// SPDX-License-Identifier: MIT OR Apache-2.0
```

```toml
# SPDX-FileCopyrightText: 2026 ccidkit contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0
```

Markdown documents, `Cargo.lock`, and `.github/CODEOWNERS` are the exceptions: they
carry no header and are covered in bulk by `REUSE.toml`. If you add a file type that
cannot carry a comment, annotate it there rather than leaving it uncovered. A captured
trace of someone else's firmware gets its own annotation recording where it came from —
it is not this project's creative work to blanket-license.

## Reading other stacks is allowed, with care

The CCID specification is public and decides any disagreement. `pcsc-lite` and CCID
(libccid) are readable and worth reading — but libccid is exactly the thing whose
license terms must be honored, and this workspace is MIT or Apache-2.0: **do not paste
from it**, and do not transcribe its quirk conclusions (see above). The `pcsc` and
`nusb` crates are dependencies, not references to copy from.

## Spelling

`typos` runs with `locale = "en-us"` and adding a word to its allowlist to silence it is
not an option. Write US spellings everywhere. The allowlist exists for a narrower thing:
a name the domain chose — a field the CCID specification spells, a reader's marketing
name — not a word this project misspelled.

## Code and comments are in English

The repository, including comments and documentation, is written in concise English so
the spell checker works and so adopters can read it.

## Commits

Conventional Commits, validated by `committed` in the commit-msg hook:

```text
feat(proto): decode the CCID class descriptor's dwFeatures word
fix(backend-usb): honor wMaxPacketSize when splitting a bulk-out transfer
docs(adr): record that a status word is data
```
