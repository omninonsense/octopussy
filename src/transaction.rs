use rust_decimal::Decimal;

pub type TransactionId = u32;
pub type ClientId = u16;

#[derive(Debug)]
pub enum TransactionEvent {
    Deposit {
        tx: TransactionId,
        client: ClientId,
        amount: Decimal,
    },

    Withdrawal {
        tx: TransactionId,
        client: ClientId,
        amount: Decimal,
    },

    Dispute {
        tx: TransactionId,
        client: ClientId,
    },

    Resolve {
        tx: TransactionId,
        client: ClientId,
    },

    Chargeback {
        tx: TransactionId,
        client: ClientId,
    },
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum TransactionError {
    #[error("client {client_id} does not exist")]
    ClientNotFound { client_id: ClientId },

    #[error(
        "client {client_id} does not have sufficient funds ({available}) to process withdrawal transaction {transaction_id} for {amount}"
    )]
    InsufficientFunds {
        client_id: ClientId,
        transaction_id: TransactionId,
        available: Decimal,
        amount: Decimal,
    },

    #[error("client {client_id}'s account is frozen")]
    AccountFrozen { client_id: ClientId },

    #[error("transaction {transaction_id} is already disputed")]
    AlreadyDisputed {
        client_id: ClientId,
        transaction_id: TransactionId,
    },

    #[error("transaction {transaction_id} is not disputed")]
    NotDisputed {
        client_id: ClientId,
        transaction_id: TransactionId,
    },

    #[error("transaction {transaction_id} does not exist")]
    TransactionNotFound {
        client_id: ClientId,
        transaction_id: TransactionId,
    },

    #[error("duplicate transaction {transaction_id}")]
    DuplicateTransaction {
        client_id: ClientId,
        transaction_id: TransactionId,
    },
}

pub struct ClientInformation {
    pub id: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub frozen: bool,
}

pub trait TransactionProcessor {
    fn process_transaction_event(
        &mut self,
        transaction: TransactionEvent,
    ) -> Result<(), TransactionError> {
        match transaction {
            TransactionEvent::Deposit { tx, client, amount } => self.deposit(tx, client, amount),
            TransactionEvent::Withdrawal { tx, client, amount } => {
                self.withdrawal(tx, client, amount)
            }
            TransactionEvent::Dispute { tx, client } => self.dispute(tx, client),
            TransactionEvent::Resolve { tx, client } => self.resolve(tx, client),
            TransactionEvent::Chargeback { tx, client } => self.chargeback(tx, client),
        }
    }

    /// Called to process the `deposit` event.
    ///
    /// If user client not exist, this should lazily create the client.
    ///
    /// If the transaction is valid, it is recorded and the user's available
    /// balance is increased by the amount.
    ///
    /// ## Errors
    /// - In case of duplicate transactions, returns [`TransactionError::DuplicateTransaction`]
    /// - In case of frozen client account, returns [`TransactionError::AccountFrozen`]
    fn deposit(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Decimal,
    ) -> Result<(), TransactionError>;

    /// Called to process the `withdrawal` event.
    ///
    /// If the transaction is valid, it is recorded and the user's available
    /// balance is decreased by the amount.
    ///
    /// ## Errors
    /// - In case the client doesn't exist, returns [`TransactionError::ClientNotFound`]
    /// - In case of duplicate transactions, returns [`TransactionError::DuplicateTransaction`]
    /// - In case of frozen client account, returns [`TransactionError::AccountFrozen`]
    /// - In case of insufficient available funds, returns [`TransactionError::InsuffucientFunds`]
    fn withdrawal(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
        amount: Decimal,
    ) -> Result<(), TransactionError>;

    /// Called when processing `dispute` events.
    ///
    /// If a transaction reference is valid, its original amount will be held on the
    /// client's account.
    ///
    /// ## Errors
    /// - If the client does not exist, returns [`TransactionError::ClientNotFound`]
    /// - If the transaction does not exist, returns [`TransactionError::TransactionNotFound`]
    /// - If the transaction is already disputed, returns [`TransactionError::AlreadyDisptuted`]
    fn dispute(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError>;

    /// Called when processing `resolve` events.
    ///
    /// If a transaction reference is valid, its original amount will no longer be disputed,
    /// and its original amount will become available again.
    ///
    /// ## Errors
    /// - If the client does not exist, returns [`TransactionError::ClientNotFound`]
    /// - If the transaction does not exist, returns [`TransactionError::TransactionNotFound`]
    /// - If the transation is not disputed, returns [`TransactionError::NotDisputed`]
    fn resolve(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError>;

    /// Called when processing `chargeback` events.
    ///
    /// If a transaction reference is valid, its original amount will be removed from the client's
    /// account, and the account will be frozen/blocked. Once the account is frozen no further
    /// deposits or withdrawals are possible.
    ///
    /// ## Errors
    /// - If the client does not exist, returns [`TransactionError::ClientNotFound`]
    /// - If the transaction does not exist, returns [`TransactionError::TransactionNotFound`]
    /// - If the transation is not disputed, returns [`TransactionError::NotDisputed`]
    fn chargeback(
        &mut self,
        transaction_id: TransactionId,
        client_id: ClientId,
    ) -> Result<(), TransactionError>;

    /// Iterator over all the clients tracked by the transaction DB.
    ///
    /// This is kinda cheating... While it's possible to have something like this
    /// in prod, but it's also not very likely. But it's a take home task, so
    /// c'est la vie.
    fn clients_iter(&self) -> impl Iterator<Item = ClientInformation>;
}
