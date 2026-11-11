#![no_main]

use libfuzzer_sys::fuzz_target;
use libfuzzer_sys::arbitrary::{Arbitrary, Unstructured};
use bitcoin::{FeeRate, ScriptBuf};
use bitcoin_transaction_builder::p2tr_transaction::from_coins;
use bitcoin::secp256k1::{rand, Secp256k1, SecretKey};
use bitcoin::key::Keypair;
use bitcoin_transaction_builder::coin::Coin;

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(&data);
    let fee_rate = FeeRate::arbitrary(&mut u).unwrap();
    let recipient = ScriptBuf::arbitrary(&mut u).unwrap();

    let coins: Vec<Coin> = Vec::arbitrary(&mut u).unwrap();

    // create Arbitrary key_pair
    let s = Secp256k1::new();
    let sk = SecretKey::new(&mut rand::thread_rng());
    let key_pair = Keypair::from_secret_key(&s, &sk);

    let _tx = from_coins(&coins, &recipient, key_pair, fee_rate);
});
