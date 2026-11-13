//! Builds a Bitcoin Transaction from a Coin.
//!
//! Creates a transaction from a coin using the function call `from_coins()`, then broadcasts the
//! transaction over the Signet Network.
//!
//! To run, provide the required positional args, for example:
//! `cargo run --example from-coin
//!     <keys-path> <fee-rate> <recipient-address> <txid> <vout> <out_amount>`
//!
//! # Arguments:
/// * `keys-path` - the path to a stored bitcoin private key.
/// * `fee-rate` - the fee-rate in sats per vB.
/// * `recipient-address` - the recipient bitcoin address.
/// * `txid` - the txid of an unspent output.
/// * `vout` - the index of the output that corresponds to the txid.
/// * `out_amount` - the amount of the output being spent (this must be exact).
use std::str::FromStr;
use std::{env, fs, process};

use bitcoin::key::Keypair;
use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{Address, Amount, FeeRate, Network, OutPoint, ScriptBuf, TxOut, Txid};
use bitcoin_transaction_builder::coin::Coin;
use bitcoin_transaction_builder::p2tr_transaction::from_coins;

use esplora_client::Builder;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 6 {
        eprintln!(
            "Usage: {} <keys-path> <fee-rate> <recipeint-address> <txid> <vout> <out-amount>",
            args[0]
        );
        process::exit(1);
    }

    let keys_path = &args[1];

    let s = Secp256k1::new();
    let bytes: Vec<u8> = fs::read(keys_path).unwrap();
    let sk = SecretKey::from_slice(&bytes).unwrap();
    let key_pair = Keypair::from_secret_key(&s, &sk);
    let sender_script_pubkey = ScriptBuf::new_p2tr(&s, key_pair.x_only_public_key().0, None);

    let str_fee_rate = &args[2];
    let fee_rate: u32 = str_fee_rate.parse::<u32>().expect("valid fee_rate");
    let fee_rate = FeeRate::from_sat_per_vb_u32(fee_rate);

    let str_addr = &args[3];
    let recipient_address: Address =
        Address::from_str(str_addr).unwrap().require_network(Network::Signet).unwrap();
    let recipient_script = recipient_address.script_pubkey();

    // blockchain location.
    let txid_str = &args[4];
    let txid = Txid::from_str(txid_str).unwrap();

    let str_vout = &args[5];
    let vout: u32 = str_vout.parse::<u32>().expect("valid vout");
    let outpoint = OutPoint { txid, vout };

    let str_amount = &args[6];
    let amount = str_amount.parse::<u64>().expect("valid amount");
    let value = Amount::from_sat(amount);
    let tx_out = TxOut { value, script_pubkey: sender_script_pubkey.clone() };
    let coin = Coin { outpoint, tx_out };
    let coins = vec![coin];

    let tx = from_coins(&coins, &recipient_script, key_pair, fee_rate);

    if let Some(t) = tx {
        println!("broadcasting: {:?}", t);
        let builder = Builder::new("https://blockstream.info/signet/api");
        let blocking_client = builder.build_blocking();
        let response = blocking_client.broadcast(&t).unwrap();
        println!("broadcast response: {:#?}", response);
    } else {
        eprintln!("error building transaction.");
    }
}
