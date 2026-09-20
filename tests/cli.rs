use std::collections::HashMap;
use std::process::Command;

use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

/// Parses an accounts CSV into a client -> (available, held, total, locked)
/// map, so comparisons are independent of row order and column order —
/// deserializing by field name (not position) is what makes column order
/// not matter.
fn parse_accounts(csv_text: &str) -> HashMap<u16, (Decimal, Decimal, Decimal, bool)> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv_text.as_bytes());
    let mut rows = HashMap::new();
    for record in reader.deserialize::<Row>() {
        let row = record.expect("fixture row should parse");
        rows.insert(row.client, (row.available, row.held, row.total, row.locked));
    }
    rows
}

fn run_fixture(name: &str) {
    let input_path = format!("tests/fixtures/{name}.csv");
    let expected = std::fs::read_to_string(format!("tests/fixtures/{name}.expected.csv"))
        .expect("expected fixture should exist");

    let output = Command::new(env!("CARGO_BIN_EXE_crabonomics"))
        .arg(&input_path)
        .output()
        .expect("binary should run");
    assert!(output.status.success(), "binary exited with an error");

    let actual = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert_eq!(parse_accounts(&actual), parse_accounts(&expected));
}

#[test]
fn basic_fixture_matches_expected_output() {
    run_fixture("basic");
}

#[test]
fn disputes_fixture_matches_expected_output() {
    run_fixture("disputes");
}
