use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::transaction::{
    ClientId, ClientInformation, TransactionError, TransactionId, TransactionProcessor,
};

/// A simplified transaction representation.
/// A disputed transaction means its amount is held rather than available.
///
/// For deposits the transaction amount is positive, while for withdrawals
/// it's negative. This simplifes things slightly
struct TransactionState {
    /// The amount of the transaction.
    ///
    /// Positive amounts represent deposits.
    /// Negative amounts represent withdrawals.
    amount: Decimal,

    // Whether the transaction is disputed or not
    disputed: bool,
}

#[derive(Default)]
pub struct ClientState {
    available: Decimal,
    held: Decimal,
    frozen: bool,
}

impl ClientState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The client's available amount
    pub fn available(&self) -> Decimal {
        self.available
    }

    /// Any amount of money that's being held due to disputed transactions
    pub fn held(&self) -> Decimal {
        self.held
    }

    /// The sum of available and held (disputed) amount on the account
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    /// Whether the client's account is frozen due to a dispute which
    /// resulted in a chargeback
    pub fn frozen(&self) -> bool {
        self.frozen
    }
}

#[derive(Default)]
pub struct InMemoryTransactionDb {
    clients: HashMap<ClientId, ClientState>,
    transaction_history: HashMap<(ClientId, TransactionId), TransactionState>,
}

impl InMemoryTransactionDb {
    pub fn new() -> Self {
        InMemoryTransactionDb::default()
    }
}

impl InMemoryTransactionDb {
    /// Used as a pre-flight check before processing deposit/withdrawal. If the transaction was
    /// already recorded it returns [`TransactionError::DuplicateTransaction`]
    pub fn ensure_transaction_uniqe(
        &self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError> {
        if self
            .transaction_history
            .contains_key(&(client_id, transaction_id))
        {
            Err(TransactionError::DuplicateTransaction {
                client_id,
                transaction_id,
            })
        } else {
            Ok(())
        }
    }
}

impl TransactionProcessor for InMemoryTransactionDb {
    fn deposit(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Decimal,
    ) -> Result<(), TransactionError> {
        self.ensure_transaction_uniqe(transaction_id, client_id)?;

        let client = self.clients.entry(client_id).or_default();

        if client.frozen {
            return Err(TransactionError::AccountFrozen { client_id });
        }

        self.transaction_history.insert(
            (client_id, transaction_id),
            TransactionState {
                amount,
                disputed: false,
            },
        );

        client.available += amount;

        Ok(())
    }

    fn withdrawal(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Decimal,
    ) -> Result<(), TransactionError> {
        self.ensure_transaction_uniqe(transaction_id, client_id)?;

        let client = self
            .clients
            .get_mut(&client_id)
            .ok_or(TransactionError::ClientNotFound { client_id })?;

        if client.frozen {
            return Err(TransactionError::AccountFrozen { client_id });
        }

        if client.available < amount {
            return Err(TransactionError::InsufficientFunds {
                client_id,
                transaction_id,
                amount,
                available: client.available(),
            });
        }

        self.transaction_history.insert(
            (client_id, transaction_id),
            TransactionState {
                amount: -amount,
                disputed: false,
            },
        );

        client.available -= amount;

        Ok(())
    }

    fn dispute(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError> {
        let client = self
            .clients
            .get_mut(&client_id)
            .ok_or(TransactionError::ClientNotFound { client_id })?;

        let transaction = self
            .transaction_history
            .get_mut(&(client_id, transaction_id))
            .ok_or(TransactionError::TransactionNotFound {
                client_id,
                transaction_id,
            })?;

        if transaction.disputed {
            return Err(TransactionError::AlreadyDisputed {
                client_id,
                transaction_id,
            });
        }

        transaction.disputed = true;
        client.available -= transaction.amount;
        client.held += transaction.amount;

        Ok(())
    }

    fn resolve(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError> {
        let client = self
            .clients
            .get_mut(&client_id)
            .ok_or(TransactionError::ClientNotFound { client_id })?;

        let transaction = self
            .transaction_history
            .get_mut(&(client_id, transaction_id))
            .ok_or(TransactionError::TransactionNotFound {
                client_id,
                transaction_id,
            })?;

        if !transaction.disputed {
            return Err(TransactionError::NotDisputed {
                client_id,
                transaction_id,
            });
        }

        transaction.disputed = false;
        client.available += transaction.amount;
        client.held -= transaction.amount;

        Ok(())
    }

    fn chargeback(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError> {
        let client = self
            .clients
            .get_mut(&client_id)
            .ok_or(TransactionError::ClientNotFound { client_id })?;

        let transaction = self
            .transaction_history
            .get_mut(&(client_id, transaction_id))
            .ok_or(TransactionError::TransactionNotFound {
                client_id,
                transaction_id,
            })?;

        if !transaction.disputed {
            return Err(TransactionError::NotDisputed {
                client_id,
                transaction_id,
            });
        }

        client.held -= transaction.amount;
        client.frozen = true;

        Ok(())
    }

    fn clients_iter(&self) -> impl Iterator<Item = ClientInformation> {
        self.clients.iter().map(|(&id, client)| ClientInformation {
            id,
            available: client.available(),
            held: client.held(),
            total: client.total(),
            frozen: client.frozen(),
        })
    }
}

#[cfg(test)]
mod test {
    use rust_decimal::dec;

    use super::*;

    #[test]
    fn deposit() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();

        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(10));
    }

    #[test]
    fn withdraw() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.withdrawal(2, 1, dec!(5)).unwrap();

        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(5));
    }

    #[test]
    fn err_insufficient_funds() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();

        let res = db.withdrawal(2, 1, dec!(11));

        assert_eq!(
            res,
            Err(TransactionError::InsufficientFunds {
                client_id: 1,
                transaction_id: 2,
                available: dec!(10),
                amount: dec!(11)
            })
        );
    }

    #[test]
    fn err_duplicate_transaction() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();

        let res = db.deposit(1, 1, dec!(10));

        assert_eq!(
            res,
            Err(TransactionError::DuplicateTransaction {
                transaction_id: 1,
                client_id: 1
            })
        );
    }

    #[test]
    fn err_no_transaction() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();

        let res = db.dispute(2, 1);
        assert_eq!(
            res,
            Err(TransactionError::TransactionNotFound {
                transaction_id: 2,
                client_id: 1
            })
        );

        let res = db.resolve(3, 1);
        assert_eq!(
            res,
            Err(TransactionError::TransactionNotFound {
                transaction_id: 3,
                client_id: 1
            })
        );

        let res = db.chargeback(4, 1);
        assert_eq!(
            res,
            Err(TransactionError::TransactionNotFound {
                transaction_id: 4,
                client_id: 1
            })
        );
    }

    #[test]
    fn err_no_client() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();

        let res = db.withdrawal(3, 3, dec!(1));
        assert_eq!(res, Err(TransactionError::ClientNotFound { client_id: 3 }));

        let res = db.dispute(3, 3);
        assert_eq!(res, Err(TransactionError::ClientNotFound { client_id: 3 }));

        let res = db.resolve(4, 4);
        assert_eq!(res, Err(TransactionError::ClientNotFound { client_id: 4 }));

        let res = db.chargeback(5, 5);
        assert_eq!(res, Err(TransactionError::ClientNotFound { client_id: 5 }));
    }

    #[test]
    fn dispute() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();
        db.dispute(2, 1).unwrap();

        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(10));
        assert_eq!(client_1.held, dec!(5));
        assert!(db.transaction_history.get(&(1, 2)).unwrap().disputed);
    }

    #[test]
    fn err_not_disputed() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();

        let res = db.resolve(2, 1);
        assert_eq!(
            res,
            Err(TransactionError::NotDisputed {
                transaction_id: 2,
                client_id: 1
            })
        );

        let res = db.chargeback(2, 1);
        assert_eq!(
            res,
            Err(TransactionError::NotDisputed {
                transaction_id: 2,
                client_id: 1
            })
        );
    }

    #[test]
    fn err_already_disputed() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();

        db.dispute(2, 1).unwrap();
        let res = db.dispute(2, 1);
        assert_eq!(
            res,
            Err(TransactionError::AlreadyDisputed {
                transaction_id: 2,
                client_id: 1
            })
        );
    }

    #[test]
    fn resolve() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();
        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(15));
        assert_eq!(client_1.held, dec!(0));

        db.dispute(2, 1).unwrap();
        let client_1 = db.clients.get(&1).unwrap();

        assert_eq!(client_1.available, dec!(10));
        assert_eq!(client_1.held, dec!(5));
        assert!(db.transaction_history.get(&(1, 2)).unwrap().disputed);

        db.resolve(2, 1).unwrap();
        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(15));
        assert_eq!(client_1.held, dec!(0));
        assert!(!db.transaction_history.get(&(1, 2)).unwrap().disputed);
    }

    #[test]
    fn chargeback() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();
        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(15));
        assert_eq!(client_1.held, dec!(0));

        db.dispute(2, 1).unwrap();
        let client_1 = db.clients.get(&1).unwrap();

        assert_eq!(client_1.available, dec!(10));
        assert_eq!(client_1.held, dec!(5));
        assert!(db.transaction_history.get(&(1, 2)).unwrap().disputed);

        db.chargeback(2, 1).unwrap();
        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.available, dec!(10));
        assert_eq!(client_1.held, dec!(0));
        assert!(db.transaction_history.get(&(1, 2)).unwrap().disputed);
        assert!(client_1.frozen);
    }

    #[test]
    fn err_account_frozen() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();
        db.dispute(2, 1).unwrap();
        db.chargeback(2, 1).unwrap();

        let res = db.deposit(3, 1, dec!(10));
        assert_eq!(res, Err(TransactionError::AccountFrozen { client_id: 1 }));
    }

    #[test]
    fn total() {
        let mut db = InMemoryTransactionDb::new();
        db.deposit(1, 1, dec!(10)).unwrap();
        db.deposit(2, 1, dec!(5)).unwrap();

        db.deposit(3, 2, dec!(10)).unwrap();
        db.deposit(4, 2, dec!(5)).unwrap();
        db.dispute(3, 2).unwrap();

        let client_1 = db.clients.get(&1).unwrap();
        assert_eq!(client_1.total(), dec!(15));

        let client_2 = db.clients.get(&2).unwrap();
        assert_eq!(client_2.total(), dec!(15));
    }
}
