//! # A p2tr Transaction Builder
//!
//! A Transaction builder for Bitcoin p2tr Transactions.

use bitcoin::absolute::LockTime;
use bitcoin::hashes::Hash;
use bitcoin::key::{Keypair, TapTweak, TweakedKeypair};
use bitcoin::secp256k1::{Message, Secp256k1};
use bitcoin::sighash::{Prevouts, SighashCache};
use bitcoin::taproot::Signature;
use bitcoin::transaction::{InputWeightPrediction, Version};
use bitcoin::{Amount, FeeRate, ScriptBuf, TapSighashType, Transaction, TxIn, TxOut};

use crate::coin::Coin;

fn calculate_tx_fee(fee_rate: FeeRate, inputs: &[TxIn], outputs: &[&ScriptBuf]) -> Option<Amount> {
    let input_iwp = InputWeightPrediction::P2TR_KEY_DEFAULT_SIGHASH;
    let input_iwps: Vec<_> = inputs.iter().map(|_| input_iwp).collect();
    let lens: Vec<_> = outputs.iter().map(|o| o.len()).collect();
    let predicted_tx_weight = bitcoin::transaction::predict_weight(input_iwps, lens);

    FeeRate::fee_wu(fee_rate, predicted_tx_weight)
}

	
/// Build a transaction from a list of coins.
///
/// All coins supplied as argument will be converted to transaction inputs.  No change output will
/// be created, and in so doing, assigns all subsequent coin value to the recipient.
///
/// # Parameters
///
/// * `coins` - the coins which will become transaction inputs.
/// * `recipient_script` - the recipient who can spend the received transaction.
/// * `keypair` - the keys needed for signing on transaction broadcast.
/// * `fee_rate` - the broadcast fee_rate.
///
/// # Returns
///
/// Returns an Option type which either wraps a successfully built transaction, or is None if
/// something unexpected happened, such as an overflow.  In a future revision, this will likely
/// change to a Result type to carry more specific information about the failure case.
pub fn from_coins(
    coins: &[Coin],
    recipient_script: &ScriptBuf,
    keypair: Keypair,
    fee_rate: FeeRate,
) -> Option<Transaction> {
    let inputs: Vec<TxIn> = coins.iter().map(|c| c.into_input()).collect();
    let tx_fee = calculate_tx_fee(fee_rate, &inputs, &[recipient_script])?;

    let prevouts: Vec<_> = coins.iter().map(|coin| coin.tx_out.clone()).collect();
    let value =
        prevouts.iter().map(|tx_out| tx_out.value).try_fold(Amount::ZERO, Amount::checked_add)?;

    let to_recipient = value.checked_sub(tx_fee)?;
    let output = TxOut { value: to_recipient, script_pubkey: recipient_script.clone() };

    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs,
        output: vec![output],
    };

    let mut sighasher = SighashCache::new(&mut tx);
    for (index, _) in coins.iter().enumerate() {
        let sighash_type = TapSighashType::Default;
        let prevouts = Prevouts::All(&prevouts);

        let sighash =
            sighasher.taproot_key_spend_signature_hash(index, &prevouts, sighash_type).unwrap();

        let s = Secp256k1::new();
        let tweaked: TweakedKeypair = keypair.tap_tweak(&s, None);
        let msg = Message::from_digest(sighash.to_byte_array());
        let signature = s.sign_schnorr(&msg, &tweaked.to_keypair());

        let signature = Signature { signature, sighash_type };
        sighasher.witness_mut(index).unwrap().push(signature.to_vec());
    }
    let tx = sighasher.into_transaction();
    Some(tx.to_owned())
}

#[cfg(test)]
mod tests {
    use arbtest::arbitrary::{Arbitrary, Result, Unstructured};
    use arbtest::arbtest;
    use bitcoin::amount::CheckedSum;
    use bitcoin::secp256k1::SecretKey;
    use bitcoin::{OutPoint, ScriptBuf};

    use super::*;
    use crate::tests::valued_coins;

    fn kp() -> Keypair {
        let s = Secp256k1::new();
        let sk = SecretKey::from_slice(&[0xcd; 32]).expect("32 bytes, within curve order");
        Keypair::from_secret_key(&s, &sk)
    }

    impl<'a> Arbitrary<'a> for Coin {
        fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
            let outpoint: OutPoint = u.arbitrary()?;
            let tx_out: TxOut = u.arbitrary()?;
            let coin = Coin { outpoint, tx_out };
            Ok(coin)
        }
    }

    #[test]
    fn from_two_coins() {
        let coins = valued_coins(&[1100, 1100]);
        let fee_rate = FeeRate::from_sat_per_vb(5).unwrap();
        let script_pubkey = &coins[0].tx_out.script_pubkey;
        let tx = from_coins(&coins, script_pubkey, kp(), fee_rate).unwrap();
        let coin_val: Amount = coins.iter().map(|c| c.tx_out.value).sum();
        let inputs: Vec<TxIn> = coins.iter().map(|c| c.into_input()).collect();
        let fee = calculate_tx_fee(fee_rate, &inputs, &[script_pubkey]);
        let output = &tx.output[0];
        assert_eq!(output.value, coin_val - fee.unwrap());
    }

    #[test]
    fn from_coins_with_invalid_fee_rate() {
        let coins = valued_coins(&[1100]);
        let fee_rate = FeeRate::MAX;
        let tx = from_coins(&coins, &ScriptBuf::new(), kp(), fee_rate);
        assert!(tx.is_none());
    }

    #[test]
    fn val_less_than_fee() {
        let fee_rate = FeeRate::from_sat_per_vb(10).unwrap();
        // tx fee of 1 tr input and 1 tr change ouput
        let tx_fee = 770;
        let coins = valued_coins(&[tx_fee - 1]);
        let tx = from_coins(&coins, &ScriptBuf::new(), kp(), fee_rate);
        assert!(tx.is_none());
    }

    #[test]
    fn arb_from_coin() {
        arbtest(|u| {
            let recipient = ScriptBuf::arbitrary(u)?;
            let coins: Vec<Coin> = Vec::arbitrary(u)?;
            let fee_rate: FeeRate = u.arbitrary()?;
            let tx = from_coins(&coins, &recipient, kp(), fee_rate);
            match tx {
                Some(t) => {
                    assert_eq!(t.input.len(), coins.len());
                    assert_eq!(t.output.len(), 1);
                    let inputs = &t.input;
                    let output = &t.output[0];
                    let input_outs: Vec<OutPoint> =
                        inputs.iter().map(|i| i.previous_output).collect();
                    let coin_outs: Vec<OutPoint> = coins.iter().map(|c| c.outpoint).collect();
                    assert_eq!(input_outs, coin_outs);
                    let coin_val: Amount = coins.iter().map(|c| c.tx_out.value).sum();
                    let tx_fee = calculate_tx_fee(fee_rate, inputs, &[&recipient]).unwrap();
                    assert_eq!(output.value, coin_val - tx_fee);
                }
                None => {
                    let inputs: Vec<TxIn> = coins.iter().map(|c| c.into_input()).collect();
                    let tx_fee = calculate_tx_fee(fee_rate, &inputs, &[&recipient]);
                    let val = coins.iter().map(|c| c.tx_out.value).checked_sum();
                    if let Some(f) = tx_fee {
                        if let Some(v) = val {
                            assert!(v < f)
                        }
                    }
                }
            }
            Ok(())
        });
    }
}
