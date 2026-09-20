---
description: Run quality control pipeline (Rust)
context: fork
allowed-tools:
  - Bash
  - TodoWrite
---

# Pipeline

Standalone Rust crate (no workspace). Single unit: `rust` at repo root. Mirrors `.github/workflows/ci.yml` exactly: fmt, clippy, test, build.

Use TodoWrite to track phases.

## Arguments

- `all` — run everything (default if no changes detected)
- `rust` — run the rust unit only (there is only one unit, so this is equivalent to `all`)

## Change Detection

This is a single-stack, single-unit repo — skip change detection entirely and just run everything.

## Rust (standalone crate)

### Phase 1: Format check

    cd $(git rev-parse --show-toplevel) && cargo fmt --check

On failure: auto-fix with `cargo fmt --all`, then re-check.

### Phase 2: Clippy

    cd $(git rev-parse --show-toplevel) && cargo clippy --locked --quiet -- -D warnings

On failure: fix and retry (max 3).

### Phase 3: Test

    cd $(git rev-parse --show-toplevel) && cargo test --locked --quiet

On failure: report to user.

### Phase 4: Build

    cd $(git rev-parse --show-toplevel) && cargo build --locked --quiet

On failure: report to user.

## Execution Order

1. Format check
2. Clippy
3. Test
4. Build

## Reporting

**On success:**
Pipeline passed — rust all clean.

**On failure:**
Pipeline failed:
- rust/<phase>: <error summary>

Keep it short — just pass/fail and what broke.
