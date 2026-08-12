# SPDX-FileCopyrightText: 2026 ccidkit contributors
#
# SPDX-License-Identifier: MIT OR Apache-2.0

# `just --list` shows only the comment line directly above a recipe, so rationale goes in
# a block above a blank line and the line touching the recipe is its one-line summary.

set shell := ["sh", "-cu"]
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

export RUSTDOCFLAGS := "-D warnings"

# No `no-std` or `wasm` lane: the public effect model intentionally uses std threads,
# while parser/protocol modules remain sans-I/O under `just purity` (ADR-0016).

# List the available development commands.
default:
    @just --list

# Format the workspace.
fmt:
    cargo fmt --all

# Check formatting without modifying files.
fmt-check:
    cargo fmt --all --check

# Check TOML formatting.
toml-check:
    taplo fmt --check --diff

# Run Clippy with and without default features across every target.
lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy --workspace --all-targets --no-default-features -- -D warnings

# Nextest runs normal tests process-per-test; Cargo separately runs doctests, which
# nextest does not currently support.

# Run the workspace test suite.
test:
    cargo nextest run --workspace --all-features
    cargo test --workspace --doc --all-features

# Run the workspace test suite with the non-fail-fast CI profile.
test-ci:
    cargo nextest run --profile ci --workspace --all-features
    cargo test --workspace --doc --all-features

# Build public documentation with warnings denied.
doc:
    cargo doc --workspace --all-features --no-deps

# Compile no-default, every individual feature, and feature pairs.
feature-matrix:
    cargo hack check --workspace --all-targets --each-feature
    cargo hack check --workspace --feature-powerset --depth 2

# Keep parser/protocol source free of I/O, clock, workers, and backend imports.
purity:
    cargo run --quiet -p xtask -- purity

# Check every member's edges against the ALLOWED matrix (docs/adr/0013).
deps:
    cargo run --quiet -p xtask -- deps

# Reject unsafe blocks and unsafe functions anywhere in repository Rust source.
unsafe-boundary:
    cargo run --quiet -p xtask -- unsafe-boundary

# Validate the packaged quirk table: order, uniqueness, provenance, and flag vocabulary.
quirkdb:
    cargo run --quiet -p xtask -- quirkdb

# Reject a bin target that shares a name with any lib crate (docs/adr/0015).
bin-name:
    cargo run --quiet -p xtask -- bin-name

# Spell-check the repository.
typos:
    typos

# Check dependency advisories, bans, licenses, and sources.
deny:
    cargo deny --all-features check advisories bans licenses sources

# Reject unused, misplaced, and unlinked Cargo dependencies or source files.
shear:
    cargo shear --deny-warnings

# Check REUSE/SPDX compliance.
reuse:
    uvx --with charset-normalizer==3.4.9 reuse==6.2.0 lint

# Validate GitHub Actions workflows.
actionlint:
    actionlint -color

# The auditor gets neither network nor repository credentials.

# Reject high-severity GitHub Actions and Dependabot security findings.
zizmor:
    zizmor --offline --persona regular --min-severity high .

# Verify every workspace crate at the shared declared MSRV.
msrv:
    cargo msrv verify --path crates/ccidkit
    cargo msrv verify --path crates/ccid-cli
    cargo msrv verify --path crates/ccid-driverkit
    cargo msrv verify --path xtask

# Mutation-test only validation and state transitions whose mistakes are protocol bugs.
mutants:
    cargo mutants -p ccidkit --all-features \
        -f 'crates/ccidkit/src/model.rs' \
        -f 'crates/ccidkit/src/ccid.rs' \
        -f 'crates/ccidkit/src/protocol.rs' \
        -f 'crates/ccidkit/src/facade.rs' \
        -F 'Command::(from_bytes|to_bytes)|Atr::parse|parse_class_descriptor|DeviceMessage::(decode|validate_for|transport_parameters)|Block::(encode|decode)|T1Machine::(new|start|accept)|transmit_automatic' \
        --minimum-test-timeout 20

# Fast deterministic checks used during the edit/commit loop.
check: fmt-check toml-check typos lint purity deps unsafe-boundary quirkdb bin-name shear reuse actionlint zizmor
    @echo "fast local checks passed"

# Every practical CI gate available on a developer machine.
ci: fmt-check toml-check typos lint feature-matrix test-ci doc purity deps unsafe-boundary quirkdb bin-name mutants deny shear reuse actionlint zizmor msrv
    @echo "local CI passed"
