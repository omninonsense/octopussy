use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::transaction::{ClientId, TransactionEvent, TransactionId, TransactionProcessor};

/// Maximum decimal places to include when formatting the CSV
const DECIMAL_PLACES: u32 = 5;

#[derive(Debug, Deserialize)]
pub struct TransactionRow {
    #[serde(rename = "type")]
    transaction_type: String,
    client: ClientId,
    tx: TransactionId,
    amount: Option<Decimal>,
}

#[derive(thiserror::Error, Debug)]
pub enum CsvDecodeError {
    #[error("amount column required for deposit")]
    MissingAmount,
    #[error("unknown transaction event type {0}")]
    UnknownType(String),
}

impl TryFrom<TransactionRow> for TransactionEvent {
    type Error = CsvDecodeError;

    fn try_from(row: TransactionRow) -> Result<Self, Self::Error> {
        match row.transaction_type.as_str() {
            "deposit" => {
                let amount = row.amount.ok_or(CsvDecodeError::MissingAmount)?;
                Ok(TransactionEvent::Deposit {
                    tx: row.tx,
                    client: row.client,
                    amount,
                })
            }
            "withdrawal" => {
                let amount = row.amount.ok_or(CsvDecodeError::MissingAmount)?;
                Ok(TransactionEvent::Withdrawal {
                    tx: row.tx,
                    client: row.client,
                    amount,
                })
            }
            "dispute" => Ok(TransactionEvent::Dispute {
                tx: row.tx,
                client: row.client,
            }),
            "resolve" => Ok(TransactionEvent::Resolve {
                tx: row.tx,
                client: row.client,
            }),
            "chargeback" => Ok(TransactionEvent::Chargeback {
                tx: row.tx,
                client: row.client,
            }),
            t => Err(CsvDecodeError::UnknownType(t.to_string())),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientRow {
    pub client: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

pub fn csv_processor<R, W, DB>(
    mut csv_reader: csv::Reader<R>,
    mut csv_writer: csv::Writer<W>,
    db: &mut DB,
) -> anyhow::Result<()>
where
    R: std::io::Read,
    W: std::io::Write,
    DB: TransactionProcessor,
{
    for row in csv_reader.deserialize() {
        let transaction_row: TransactionRow = row?;
        let transaction: TransactionEvent = transaction_row.try_into()?;

        info!("Processing transaction event: {:?}", transaction);
        if let Err(err) = db.process_transaction_event(transaction) {
            error!("transaction error: {err}")
        }
    }

    for client in db.clients_iter() {
        let row = ClientRow {
            client: client.id,
            available: client.available.round_dp(DECIMAL_PLACES),
            held: client.held.round_dp(DECIMAL_PLACES),
            total: client.total.round_dp(DECIMAL_PLACES),
            locked: client.frozen,
        };

        csv_writer.serialize(row)?;
    }

    csv_writer.flush()?;

    Ok(())
}
