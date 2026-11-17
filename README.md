# Rust Bitcoin Transaction Builder

<p>
    <a href="https://crates.io/crates/bitcoin_transaction_builder"><img alt="Crate Info" src="https://img.shields.io/crates/v/bitcoin_transaction_builder.svg"/></a>
    <a href="https://github.com/yancyribbens/rust-bitcoin-transaction-builder/actions?query=workflow%3AContinuous%20integration"><img alt="CI Status" src="https://github.com/yancyribbens/rust-bitcoin-transaction-builder/workflows/Continuous%20integration/badge.svg"></a>
    <a href="https://docs.rs/bitcoin-transaction-builder"><img alt="API Docs" src="https://img.shields.io/docsrs/bitcoin-transaction-builder"/></a>
    <a href="https://blog.rust-lang.org/2023/11/16/Rust-1.56.1/"><img alt="Rustc Version 1.56.1+" src="https://img.shields.io/badge/rustc-1.56.1%2B-lightgrey.svg"/></a>
</p>

A builder for Bitcoin Transactions written in Rust.

Note that currently, only P2TR v2 transactions are supported as this is a work in progress.

## About

Provides an interface for building bitcoin transactions from either a target amount or list of coins.
If passing an amount using the function `from_amount()`, automatic coin-selection is performed to find
the cheapest combination of inputs needed to fund the transaction.  You could also perform a
coin-selection by other means, then pass all the inputs directly using `from_coins()`.  Note that
`from_coins` will never create a change output while `from_amount` may or may not.

## Example

See the [examples directory](examples/) for a full working example.

```rust
let tx = from_amount(
    &coins,                     // a list of coins to choose from.
    target_amount,              // the amount to send.
    &recipient_script_pubkey,   // recipient details.
    &sender_script_pubkey,      // sender details in case of change.
    key_pair,                   // key for transaction signing.
    fee_rate,                   // fee-rate for transaction broadcast.
);

// broadcast tx
```


## Fuzz tests

To run fuzz tests, install [cargo fuzz](https://crates.io/crates/cargo-fuzz).

The following fuzz tests can then be run:
```
> cargo fuzz run p2tr_tx_from_amount 
> cargo fuzz run p2tr_tx_from_coin 
```

## Minimum Supported Rust Version (MSRV)

This library should always compile with any combination of features on **Rust 1.56.1**.

## Release Notes

- [CHANGELOG](CHANGELOG.md)
