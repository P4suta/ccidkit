# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). No
compatibility promise is made until the first deliberate release.

## [Unreleased]

### Added

- One published `ccidkit` library with runtime-neutral blocking/async `Operation`.
- Validated APDU, response/status, ATR, CCID descriptor/message, and T=1 block models.
- Concrete `Context → Reader → Card → Transaction` ownership API and ordered workers.
- Native USB CCID, platform PC/SC, and deterministic virtual reader backends.
- Real PC/SC transaction brackets, APDU `6Cxx`/`61xx` policy, bounded sensitive traces,
  multi-slot native enumeration, and generated reader quirks.
- `ccid` application CLI and private `ccdev doctor` diagnostic command.
- Dependency, source-purity, no-unsafe, quirk-schema, compile-fail, lint, and targeted
  mutation gates.

### Changed

- Consolidated the bootstrap's prospective seven public libraries behind one minimal
  semver surface ([ADR-0016](docs/adr/0016-one-library-one-operation.md)).
