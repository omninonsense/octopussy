use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use csv::ReaderBuilder;
use octopussy::{
    csv::{ClientRow, csv_processor},
    memory_processor::InMemoryTransactionDb,
};
use tracing::info;

type ClientId = u16;

#[test]
fn test_sample_files() -> Result<(), Box<dyn Error>> {
    let samples_dir = PathBuf::from("samples");

    let sample_files = std::fs::read_dir(&samples_dir)?
        .filter_map(Result::ok)
        .filter(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(".in.csv")
        })
        .collect::<Vec<_>>();

    for entry in sample_files {
        let input_path = entry.path();
        let expected_output_path = create_expected_output_path(&input_path);

        info!("Testing sample: {}", input_path.display());

        let actual_output = process_input_file(&input_path)?;
        let actual_output_map = parse_csv_string_to_client_map(&actual_output)?;

        // Parse the expected output file
        let expected_output = parse_csv_to_client_map(&expected_output_path)?;

        // Compare the outputs semantically
        assert_eq!(
            expected_output.len(),
            actual_output_map.len(),
            "Number of clients in the output doesn't match expected for {}",
            input_path.display()
        );

        for (client_id, expected_client) in &expected_output {
            let actual_client = actual_output_map.get(client_id).unwrap_or_else(|| {
                panic!(
                    "Client {} missing from output for {}",
                    client_id,
                    input_path.display()
                )
            });

            assert_eq!(
                expected_client.available,
                actual_client.available,
                "Available amount wrong for client {} in {}. Expected: {}, Got: {}",
                client_id,
                input_path.display(),
                expected_client.available,
                actual_client.available
            );

            assert_eq!(
                expected_client.held,
                actual_client.held,
                "Held amount wrong for client {} in {}. Expected: {}, Got: {}",
                client_id,
                input_path.display(),
                expected_client.held,
                actual_client.held
            );

            assert_eq!(
                expected_client.total,
                actual_client.total,
                "Total amount wrong for client {} in {}. Expected: {}, Got: {}",
                client_id,
                input_path.display(),
                expected_client.total,
                actual_client.total
            );

            assert_eq!(
                expected_client.locked,
                actual_client.locked,
                "Locked status wrong for client {} in {}",
                client_id,
                input_path.display()
            );
        }
    }

    Ok(())
}

fn create_expected_output_path(input_path: &Path) -> PathBuf {
    let file_name = input_path.file_name().unwrap().to_string_lossy();

    let expected_file_name = file_name.replace(".in.csv", ".out.csv");

    input_path.with_file_name(expected_file_name)
}

fn process_input_file(input_path: &Path) -> Result<String, Box<dyn Error>> {
    let file = File::open(input_path)?;

    let csv_reader = ReaderBuilder::default()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(BufReader::new(file));

    let mut output_buffer = Vec::new();

    {
        let csv_writer = csv::WriterBuilder::default()
            .has_headers(true)
            .from_writer(&mut output_buffer);

        let mut db = InMemoryTransactionDb::new();

        csv_processor(csv_reader, csv_writer, &mut db)?;
    }

    let output_string = String::from_utf8(output_buffer)?;

    Ok(output_string)
}

fn parse_csv_string_to_client_map(
    csv_string: &str,
) -> Result<HashMap<ClientId, ClientRow>, Box<dyn Error>> {
    let mut reader = ReaderBuilder::default()
        .has_headers(true)
        .trim(csv::Trim::All)
        .from_reader(Cursor::new(csv_string));

    let mut clients = HashMap::new();

    for result in reader.deserialize() {
        let client: ClientRow = result?;
        clients.insert(client.client, client);
    }

    Ok(clients)
}

fn parse_csv_to_client_map(path: &Path) -> Result<HashMap<ClientId, ClientRow>, Box<dyn Error>> {
    let csv_string = std::fs::read_to_string(path).unwrap();
    parse_csv_string_to_client_map(&csv_string)
}
