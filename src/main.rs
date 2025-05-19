use std::{fs::File, io::BufReader};

use anyhow::{Context, bail};
use octopussy::{csv::csv_processor, memory_processor::InMemoryTransactionDb};
use tracing::info;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = std::env::args().collect();

    let file_path = match args.get(1) {
        Some(s) => s,
        None => {
            bail!("No file path passed to CLI");
        }
    };

    info!("Opening file file: {}", file_path);
    let file = File::open(file_path).context(format!("failed to open {file_path}"))?;

    let csv_reader = csv::ReaderBuilder::default()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(BufReader::new(file));

    let csv_writer = csv::WriterBuilder::default()
        .has_headers(true)
        .from_writer(std::io::stdout());

    let mut db = InMemoryTransactionDb::new();

    csv_processor(csv_reader, csv_writer, &mut db)?;

    Ok(())
}
