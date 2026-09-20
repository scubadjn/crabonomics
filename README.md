# crabonomics

A toy payments engine: reads a CSV of transactions, applies them to
per-client account state, writes the resulting balances back out as CSV.

## Usage

```
cargo run -- tests/fixtures/basic.csv > accounts.csv
```

The input path is the only argument (swap in your own CSV); output is
written to stdout.

## Testing

`cargo test` runs both suites: unit tests in `src/engine.rs` and
`src/types.rs` exercising the state machine and CSV-row conversion
directly, and integration tests in `tests/cli.rs` that run the compiled
binary against the fixtures in `tests/fixtures/` and diff the output
against `*.expected.csv` (row/column order ignored). `basic.csv` covers
plain deposits and withdrawals across two clients; `disputes.csv` covers
dispute → resolve, dispute → chargeback, an unknown-tx dispute, and an
insufficient-funds withdrawal. The type system also does correctness
work directly: amounts
are `rust_decimal::Decimal` (never `f64`, so 4-decimal-place inputs
round-trip exactly) and transaction rows are parsed into an enum where a
missing `amount` on deposit/withdrawal is a parse error, not a runtime
check.

## Assumptions

A few cases are left undefined by the requirements; here's how this
implementation resolves them:

- **Locked accounts reject all five transaction types** — once frozen,
  no further transactions are applied.
- **Disputes aren't restricted to a particular transaction type** —
  dispute/resolve/chargeback are defined purely as arithmetic on
  available/held/total against whatever `tx` is referenced, so the
  engine applies the same formula uniformly rather than special-casing
  by kind. In practice this means a chargeback always drains `held`
  without crediting `available` — for a disputed deposit that correctly
  discards the fraudulent funds, but for a disputed withdrawal it does
  not credit the client's withdrawn amount back.
- **A dispute/resolve/chargeback is checked against the client of the
  `tx` it references** — a reference to another client's transaction is
  treated as unknown (ignored), as a basic guard against one client
  disputing another's transaction.
- **Malformed input (bad CSV row, unknown transaction type, non-decimal
  amount) aborts the run with an error** rather than being skipped —
  treated as an upstream data problem, not a case to silently paper over.

## Safety & error handling

- Arithmetic uses `checked_add`/`checked_sub` throughout instead of
  panicking `+=`/`-=`, so a pathological overflow surfaces as an
  `anyhow::Error` instead of crashing the process.
- No `unwrap()`/`expect()` in production code paths; I/O and parse
  failures propagate via `anyhow::Result` with context (e.g. which file
  failed to open).
- The CSV reader streams records one at a time (`reader.deserialize()`
  as an iterator) rather than loading the whole file into memory, so
  throughput scales with input size rather than being bounded by RAM —
  relevant since `tx` is a `u32` and real inputs could be large.
