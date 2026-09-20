mod engine;
mod types;

use std::fs::File;
use std::io;

use anyhow::Context;
use engine::Engine;
use types::Record;

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: crabonomics <transactions.csv>")?;
    let file = File::open(&path).with_context(|| format!("opening {path}"))?;
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(file);

    let mut engine = Engine::new();
    for record in reader.deserialize::<Record>() {
        engine.apply(record?.try_into()?)?;
    }

    engine.write_csv(io::stdout())
}
