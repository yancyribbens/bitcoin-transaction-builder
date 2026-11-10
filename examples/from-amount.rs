//! Builds a Bitcoin Transaction given a target amount to send
//!
use std::fs;
use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::key::Keypair;
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, TxOut, Txid};
use bitcoin_transaction_builder::coin::Coin;
use bitcoin_transaction_builder::p2tr_transaction::from_amount;

fn main() {
    let fee_rate_arg = 5;
    let address_string = "";
    let keys_path = "";

    // txid = a valid tx_id
    // vout = the outpoint of a UTXO to spend
    // utxo_val = the value of the UTXO to spend

    let s = Secp256k1::new();
    let bytes: Vec<u8> = fs::read(keys_path).unwrap();
    let sk = SecretKey::from_slice(&bytes).unwrap();
    let key_pair = Keypair::from_secret_key(&s, &sk);
    let sender_script_pubkey = ScriptBuf::new_p2tr(&s, key_pair.x_only_public_key().0, None);

    let fee_rate = FeeRate::from_sat_per_vb_u32(fee_rate_arg);
    let address: Address =
        Address::from_str(address_string).unwrap().require_network(Network::Signet).unwrap();

    // blockchain location.
    let outpoint = OutPoint { txid: Txid::from_byte_array([1; 32]), vout: 0 };

    // amount and redeem script.
    let coin_val = Amount::from_sat(1123);
    let tx_out = TxOut { value: coin_val, script_pubkey: sender_script_pubkey.clone() };

    let coin = Coin { outpoint, tx_out };

    let target = Amount::from_sat(42);
    let _tx = from_amount(
        &[coin],
        target,
        &address.script_pubkey(),
        &sender_script_pubkey,
        key_pair,
        fee_rate,
    );

    // send tx
}
