# Bitcoin Transaction Builder

A Transaction builder for Bitcoin Transactions.

This is in many ways, a front-end for the [Rust Bitcoin Coin-Selection](https://crates.io/crates/bitcoin-coin-selection) crate.

Note that currently, only P2TR v2 transactions are supported as this is a work in progress.

## About

Provides an interface for building bitcoin transactions from either a target amount or list of coins.
If passing an amount using `from_amount()` interface, automatic coin-selection is performed to find
the cheapest combination of inputs needed to fund the transaction.  You could also perform a
coin-selection by other means, then pass all the inputs directly using `from_coins()`.  Note that
`from_coins` will never create a change output while `from_amount` may or may not.

## Example

See [examples directory](examples/) for full working example.

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

This library should always compile with any combination of features on **Rust 1.63**.

##
