# Octopussy

:octopus: :sunglasses:

## Usage

```sh
cargo run -- samples/pdf.in.csv
```

## Completeness

Wrote a few tests with samples to make sure the code works as expected.
Some semantics are encoded in the types too (discussed later in the doc).

### Assumptions

I wasn't sure whether a `dispute` can only affect `withdrawal` or both `deposit` and `withdrawal`.

I made it so it can reference either. This means that the following events can cause the client
to have a negative balance:

- deposit (tx1) 200
- withdraw (tx2) 100
- dispute tx1
- chargeback

The `available`/`total` balance is now -100.

I'm not sure if this is the correct behaviour, but I assume it is since banks allow overdrafts?

## Safety & Robustness

### Error Handling

Most things in the codebase will return a `Result<_, TransactionError>`.
I didn't think too much about the `Ok` values as they weren't really used.
So most things actually return `Result<(), TransactionError>`.

The reason why I went with this approach was that it's relatively simple, but allows
the caller to respond to errors.

In this specific case I only log the errors since the task description says it's fine to ignore
most events errors that pop up in the stream

### Safety

I've tries to encode a few things into the type system to reduce manual checking. A few example are:

- using `(ClientId, TransactionId)` for the transaction history keys
- the CSV rows (`TransactionRow`) are converted to `enum transactionEvent`
- custom error enum (`TransactionError`)
- use `rust_decimal` to avoid floating point precision issues (at the cost of some memory and performance)

For the sake of simplicity, I don't use `checked_add`/`checked_sub`... And if anyone overflows 128bits,
friggin kudos to them! :joy:

I assume the stream of events in the CSV is formatted correctly (eg amounts aren't negative, no overflows
in ids/amounts, etc). The parsing is fairly loose and laregely relies on serde.

No `unsafe` code is used.

## Efficiency

The file contents are streamed in (synchronously) via `BufReader(File)`, but it should work with
anything that implements `std::io::Read`.

The results are all stored in-memory. Since it's a take-home task and it's explicitly stated that
it must work with `cargo run -- someinput.csv`.

For a production environment it would make more sense to store it in a database (for more reasons than
just conserving memory!).

If the processor was bundled in a server, and the event came from the network it should continue to
work fine. It would need a bit of refactoring though, and parts of the code would benefit from being
made async too. And again since everything is stored in memory, a large amount of transactions could
become an issue

## Maintainability

The code is split up into a few modules:

- `csv`: holds all of the CSV-related IO
- `transaction` contains the core types and traits
- `memory_processor` has an in-memory implementation of `trait TransactionProcessor`

There's a few type aliases (ie `ClientId` and `TransactionId`) to make any potential refactors easier.
I didn't use the newtype pattern to make the task's footprint a bit smaller (and might be overkill
for something like this).

The trait should allow us to implement multiple "backends" for the transaction processor.

Using `thiserror` for more ergonomic error handling.

I didn't go over-board with the doc comments since it's just a take-home task, but I did comment some
of the the core APIs at least.
