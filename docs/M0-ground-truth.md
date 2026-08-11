# M0 ground truth

The measurements the day-one design rests on. Where a decision cites a number, it cites
this file; where this file's numbers move (upstream grows, a crate name is claimed), the
citing ADR is the place to reassess.

Measured 2026-08-10..11 on the development machine (Windows, MSVC host). Clones and
temporary projects were kept for re-verification (local scratchpad `m0-ccidkit/`:
`PCSC/`, `CCID/`, `dep-nusb/`, `dep-pcsc/`).

## What is being replaced (SLOC)

Tool: tokei 14.0.0 (via `mise x tokei@latest`), on `git clone --depth 1` at
PCSC `d3d6b25` (2026-06-21) and CCID `7bc89e3` (2026-08-10).

**pcsc-lite (PCSC), whole repo:** 18,752 SLOC total — C 12,914 (38 files) + C headers
1,123 (23) + Python 2,723 (40, tooling/tests) + Lex 464 + Autoconf 828 + misc.
`src/` only: 14,199 SLOC (C 12,395 in 31 files + headers 1,123 in 23 files + Lex 464).

**CCID driver (libccid), whole repo:** 12,050 SLOC code — C 10,163 (20 files) + C
headers 1,305 (26) + Lex 165 + Perl 91 + Python 91 + Meson 172; plus 708 plain-text
files (61,866 lines) which are almost entirely `readers/` descriptor dumps.
`src/` only: C 9,042 (16 files) + headers 899 (19) = **10,230 SLOC total**.

Reading: a Rust ccidkit replaces the CCID driver layer, not pcsc-lite's daemon and IPC —
the `pcsc` crate already covers the "use the existing stack" path (ADR 0001, 0007).

## The quirk knowledge, quantified

All from the CCID clone (grep/sed/awk):

- `readers/` holds **700** per-reader USB descriptor dump `.txt` files, plus
  `extra_features/` with **5** pinpad feature-override files (a second, tiny quirk
  channel — modeled as flags in the same table, ADR 0009). Across the dumps: 785
  idVendor/idProduct interface entries, **665 unique VID:PID pairs**.
- Info.plist generation source `src/supported_readers.txt` (fed to
  `src/create_Info_plist.pl`): **652** non-comment entries, all unique VID:PID:name
  triples.
- `ccid_open_hack` functions in `src/ccid.c`: `ccid_open_hack_pre` lines 59-187 =
  **129 lines / 16 case labels**; `ccid_open_hack_post` lines 290-638 =
  **349 lines / 27 case labels**; Gemalto firmware-feature helpers between them
  (lines 195-289) = **95 lines**. Whole hack block lines 59-638 = **580 lines**,
  per-reader case labels total **43**.
- `DRIVER_OPTION`: **4 defined flags** (`src/ccid_ifdhandler.h:37-40`):
  `CCID_EXCHANGE_AUTHORIZED 0x01`, `GEMPC_TWIN_KEY_APDU 0x02`,
  `USE_BOGUS_FIRMWARE 0x04`, `DISABLE_PIN_RETRIES 0x40`; bits 4-5
  (`0x00/0x10/0x20/0x30`) encode a voltage-sequence option documented in
  Info.plist.src (not a named define); `0x08` free.

Reading: the folklore moat is 580 lines of code plus declarative data. The data is
trivially convertible to a build-time-embedded table — but is not transcribed, for the
provenance reasons in ADR 0009.

## Dependency cost of the edges

Method: `cargo new` temp projects (`dep-nusb`, `dep-pcsc`) + `cargo add <crate>` +
`cargo tree --edges normal --prefix none | sort -u` (normal deps only, dedup by
crate@version), Windows MSVC host.

- **nusb v0.2.7:** 6 unique dependency crates on the Windows host target
  (futures-core, log, nusb, slab, windows-link, windows-sys; 7 lines incl. the root
  project). With `--target all` (union across platforms): 39 lines incl. root =
  **38 unique crates** (Linux/macOS backends pull rustix etc.). No C toolchain
  requirement anywhere — which matters on this dev machine, where pure-Rust MSVC
  builds are verified fine and native C deps are the pain point.
- **pcsc v2.9.0:** **3 unique dependency crates** (pcsc, pcsc-sys, bitflags; 4 lines
  incl. root). Identical count under `--target all` — platform-uniform and
  near-zero-dep, but it links the system PC/SC stack (winscard.dll / pcsc-lite)
  rather than replacing it.

Reading: the shim is cheap to keep first-class forever (ADR 0006, 0007); the pure
layer's zero-dependency charter carries the audit story (ADR 0013).

## crates.io namespace

All checked via `curl -A` against `https://crates.io/api/v1/crates/<name>`; HTTP 404 =
name available. **All 10 planned names are available (404):** ccidkit, ccid-core,
ccid-apdu, ccid-proto, ccid-backend-usb, ccid-backend-pcsc, ccid-backend-virtual,
ccid-cli, ccid-driverkit, ccid-testkit. Bonus: bare `ccid` is also 404 (unclaimed). No
403s observed, so these are true not-found responses, not bot-blocking (ADR 0015).

## References verified live

- USB-IF CCID specification Rev 1.1:
  <https://www.usb.org/sites/default/files/DWG_Smart-Card_CCID_Rev110.pdf>
  (HTTP 200, 3,232,269 bytes, direct PDF, no login).
- vsmartcard (vpcd/vicc — virtual reader and card for testing without hardware):
  <https://github.com/frankmorgner/vsmartcard> (HTTP 200) (ADR 0014, the M4 oracle).

## Decision-relevant observations

1. **The quirk knowledge is smaller than folklore suggests:** the entire per-reader
   hack logic is 580 lines of C with 43 reader-specific case labels and only 4
   driver-option flags. The bulk of the "reader knowledge" is the declarative
   `supported_readers.txt` (652 VID:PID entries) plus 700 descriptor dumps — data, not
   code, trivially convertible to a build-time-embedded table (the same pattern as
   bufrkit's WMO CSV embedding) (ADR 0009).
2. **The replacement target is the driver layer only:** CCID `src/` ≈ 10.2k SLOC and
   pcsc-lite `src/` ≈ 14.2k, but a Rust ccidkit only replaces the CCID bulk/interrupt
   protocol layer; the `pcsc` crate (3 deps) already covers the "use the existing
   stack" path, making `ccid-backend-pcsc` cheap (ADR 0001, 0007).
3. **nusb is genuinely lightweight on Windows** (6 crates; 38 in the all-platform
   union), with no C toolchain requirement anywhere (ADR 0001, 0013).
4. **`readers/extra_features/` holds 5 pinpad-feature override files** — a second,
   tiny quirk channel beyond `ccid_open_hack`, worth modeling in the same quirk table
   (ADR 0009).
5. The clones and temp cargo projects are kept in the local scratchpad
   (`m0-ccidkit/`) for re-verification.
