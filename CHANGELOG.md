# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Workspace bootstrap: ten crate skeletons, the quality gates, the quirk-table schema,
  and the day-one architectural decision records. No reader is spoken to yet.
- The xtask gates — `purity`, `deps`, `unsafe-boundary`, `quirkdb`, `bin-name` — written
  before the code they govern, with the ALLOWED dependency matrix transitively closed
  and self-tested.
- `quirks/readers.toml`: the reader quirk table, schema-checked from zero entries.
