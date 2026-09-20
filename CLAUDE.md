# crabonomics

A toy payments engine, built as a standalone Rust CLI. Reads a CSV of transactions (deposit, withdrawal, dispute, resolve, chargeback), applies them to per-client account state, and writes the resulting balances back out as CSV.

Domain rules, assumptions, and Rust conventions are documented in `README.md`. The `/pipeline` skill runs the same fmt/clippy/test/build gate CI runs, so agent and CI share one quality bar.

## Structure

- `src/types.rs` — domain model: client/tx ids, `Decimal`-based amounts, the `Transaction` enum, `Account` state
- `src/engine.rs` — `Engine::apply`, the state machine applying transactions to accounts (see `README.md` for the exact rules and documented assumptions)
- `src/main.rs` — CLI entry point: streaming CSV read → engine → CSV write to stdout
- `tests/cli.rs` — integration tests running the built binary against `tests/fixtures/`

## Pipeline

`/pipeline` — fmt → clippy → test → build.
