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
use bitcoin::{Amount, FeeRate, ScriptBuf, TapSighashType, Transaction, TxIn, TxOut, Weight};
use bitcoin_coin_selection::{branch_and_bound, single_random_draw};

use crate::coin::Coin;

const DEFAULT_DISCARD_FEE_RATE: FeeRate = FeeRate::from_sat_per_vb_u32(10);
const DEFAULT_LONG_TERM_FEE_RATE: FeeRate = FeeRate::from_sat_per_vb_u32(10);

// base_weight = (32 byte txid, 4 byte output index, 1 byte scriptSig, and 4 byte sequence) * 4
// tr satisfaction_weight = 66 wu
const TR_INPUT_WEIGHT: Weight = Weight::from_wu((32 + 4 + 1 + 4) * 4 + 66);
const TR_OUTPUT_WEIGHT: Weight = Weight::from_vb_unchecked(43);

// cost of change is the cost to create a change output plus the estimated cost to spend it as
// input.
//
// therefore, cost_of_change =
//      (change output size * fee rate) +
//      (change spend size * discard fee rate)
//
// The change output size of a TR output is 57.5 vB (230 wu)
// The change spend size is its input size in a future transaction, 43 vB (172 wu)
// Therefore, the total size estimate is 100.5 vB or 402 wu
//
// params
//  * fee_rate - current effective fee rate.
//  * discard_fee_rate - target fee rate is fee rate with which output will not be a dust output.
//    ref: core PR# 10817
fn default_tr_cost_of_change(fee_rate: FeeRate, discard_fee_rate: FeeRate) -> Amount {
    let change_spend_size = TR_INPUT_WEIGHT;
    let change_output_size = TR_OUTPUT_WEIGHT;

    let change_fee = fee_rate.checked_mul_by_weight(change_output_size).unwrap_or(Amount::MAX);
    let min_viable_change =
        discard_fee_rate.checked_mul_by_weight(change_spend_size).unwrap_or(Amount::MAX);
    min_viable_change.checked_add(change_fee).unwrap_or(Amount::MAX)
}

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

/// Builds a transaction from a target amount.
///
/// Uses automatic coin selection to select coins from.  The resulting transaction inputs will
/// sum to the target amount or greater.  If the input sum is greater than the target amount plus
/// the cost of creating a change output, then a change output will be created for the overage.
/// Otherwise, there will be one recipient output.
///
/// # Parameters
///
/// * `available_coins` - the possible coins used as input.  
/// * `target` - the amount the recipient will receive.
/// * `recipient_script` - the recipient redeem script.
/// * `change_script` - the change output redeem script if there is one.
/// * `keypair` - the keys needed for signing.
/// * `fee_rate` - the broadcast fee_rate.
///
/// # Returns
///
/// Returns an Option type which either wraps a successfully built transaction, or is None if
/// something unexpected happened, such as not enough coins or overflow.  In a future revision,
/// this will likely change to a Result type to carry more specific information about the failure
/// case.
pub fn from_amount(
    available_coins: &[Coin],
    target: Amount,
    recipient_script: &ScriptBuf,
    change_script: &ScriptBuf,
    keypair: Keypair,
    fee_rate: FeeRate,
) -> Option<Transaction> {
    let discard_fee_rate = DEFAULT_DISCARD_FEE_RATE;
    let lt_fee_rate = DEFAULT_LONG_TERM_FEE_RATE;
    let cost_of_change = default_tr_cost_of_change(fee_rate, discard_fee_rate);

    let base_tx_fee = calculate_tx_fee(fee_rate, &[], &[recipient_script])?;
    let total_target = target.checked_add(base_tx_fee)?;
    let bnb_result =
        branch_and_bound(total_target, cost_of_change, fee_rate, lt_fee_rate, available_coins);

    let mut output = vec![];
    let recipient_output = TxOut { value: target, script_pubkey: recipient_script.clone() };
    output.push(recipient_output);

    let inputs: Vec<_>;
    let prevouts: Vec<_>;
    if let Some((_i, selection)) = bnb_result {
        let coins: Vec<Coin> = selection.into_iter().cloned().collect();
        inputs = coins.iter().map(|c| c.into_input()).collect();

        prevouts = coins.iter().map(|coin| coin.tx_out.clone()).collect();
    } else {
        let base_tx_fee = calculate_tx_fee(fee_rate, &[], &[recipient_script, change_script])?;
        let total_target = target.checked_add(base_tx_fee)?;
        let (_, selection) = single_random_draw(total_target, fee_rate, available_coins)?;
        let coins: Vec<Coin> = selection.into_iter().cloned().collect();

        inputs = coins.iter().map(|c| c.into_input()).collect();
        let coin_sum: Amount = coins.iter().map(|c| c.tx_out.value).sum();
        let tx_fee = calculate_tx_fee(fee_rate, &inputs, &[recipient_script, change_script])?;
        let change_amt = coin_sum - target - tx_fee;
        let change_output = TxOut { value: change_amt, script_pubkey: change_script.clone() };
        output.push(change_output);

        prevouts = coins.iter().map(|coin| coin.tx_out.clone()).collect();
    }

    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs.clone(),
        output,
    };

    let mut sighasher = SighashCache::new(&mut tx);
    for (index, _) in inputs.iter().enumerate() {
        let sighash_type = TapSighashType::Default;
        let prevouts = Prevouts::All(&prevouts);

        let sighash =
            sighasher.taproot_key_spend_signature_hash(index, &prevouts, sighash_type).ok()?;

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
    use bitcoin::blockdata::transaction::effective_value;
    use bitcoin::secp256k1::SecretKey;
    use bitcoin::{OutPoint, ScriptBuf};
    use bitcoin_coin_selection::WeightedUtxo;

    use super::*;
    use crate::tests::valued_coins;

    fn kp() -> Keypair {
        let s = Secp256k1::new();
        let sk = SecretKey::from_slice(&[0xcd; 32]).expect("32 bytes, within curve order");
        Keypair::from_secret_key(&s, &sk)
    }

    fn calc_input_sum(inputs: &[TxIn], coins: &[Coin]) -> Amount {
        input_amounts(inputs, coins).into_iter().sum()
    }

    fn input_amounts(inputs: &[TxIn], coins: &[Coin]) -> Vec<Amount> {
        let mut amounts = vec![];
        inputs.iter().map(|i| i.previous_output).for_each(|prev_out| {
            let coin = coins.iter().find(|coin| coin.outpoint == prev_out).unwrap();
            amounts.push(coin.value())
        });
        amounts
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
    fn srd_target() {
        let s = Secp256k1::new();
        let coins = valued_coins(&[1123, 5812]);
        let target = Amount::from_sat(2100);
        let fee_rate = FeeRate::from_sat_per_vb(5).unwrap();
        let recipient = ScriptBuf::new_p2tr(&s, kp().x_only_public_key().0, None);
        let change_addr = recipient.clone();
        let tx = from_amount(&coins, target, &recipient, &change_addr, kp(), fee_rate).unwrap();
        let outputs = &tx.output;
        let inputs = &tx.input;
        assert_eq!(outputs[0].value, target);
        assert_eq!(2, tx.output.len());
        let input_sum = calc_input_sum(inputs, &coins);
        let tx_fee = calculate_tx_fee(fee_rate, inputs, &[&recipient, &change_addr]).unwrap();
        assert!(input_sum > target + tx_fee);
    }

    #[test]
    fn bnb_target() {
        let coins = valued_coins(&[1100]);
        let target = coins[0].value();
        let fee_rate = FeeRate::ZERO;
        let recipient = ScriptBuf::new();
        let change_addr = ScriptBuf::new();
        let tx = from_amount(&coins, target, &recipient, &change_addr, kp(), fee_rate).unwrap();
        let inputs = tx.input;
        let outputs = tx.output;

        assert!(inputs.len() == 1);
        let input_sum = calc_input_sum(&inputs, &coins);
        assert_eq!(input_sum, target);

        assert!(outputs.len() == 1);
        let output_sum: Amount = outputs.iter().map(|output| output.value).sum();
        assert_eq!(output_sum, target);

        // zero fee-rate, so there is nothing deducted for fee.
        assert_eq!(output_sum, input_sum);
    }

    #[test]
    fn from_coins_with_invalid_fee_rate() {
        let coins = valued_coins(&[1100]);
        let fee_rate = FeeRate::MAX;
        let tx = from_coins(&coins, &ScriptBuf::new(), kp(), fee_rate);
        assert!(tx.is_none());
    }

    #[test]
    fn from_amount_invalid_fee_rate() {
        let target = Amount::ZERO;
        let recipient = ScriptBuf::new();
        let change_addr = ScriptBuf::new();
        let fee_rate = FeeRate::MAX;
        let tx = from_amount(&[], target, &recipient, &change_addr, kp(), fee_rate);
        assert!(tx.is_none());
    }

    #[test]
    fn from_amount_invalid_target() {
        let target = Amount::MAX;
        let recipient = ScriptBuf::new();
        let change_addr = ScriptBuf::new();
        let fee_rate = FeeRate::from_sat_per_vb(10).unwrap();
        let tx = from_amount(&[], target, &recipient, &change_addr, kp(), fee_rate);
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
    fn no_solution() {
        let coin = vec![];
        let target = Amount::from_sat(42);
        let recipient = ScriptBuf::new();
        let change_addr = ScriptBuf::new();
        let fee_rate = FeeRate::ZERO;
        let tx = from_amount(&coin, target, &recipient, &change_addr, kp(), fee_rate);
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

    #[test]
    fn arb_from_amount() {
        arbtest(|u| {
            let coins: Vec<Coin> = Vec::arbitrary(u)?;
            let target = Amount::arbitrary(u)?;
            let recipient = ScriptBuf::arbitrary(u)?;
            let change_addr = ScriptBuf::arbitrary(u)?;
            let fee_rate: FeeRate = FeeRate::arbitrary(u)?;
            let tx = from_amount(&coins, target, &recipient, &change_addr, kp(), fee_rate);
            let cost_of_change = default_tr_cost_of_change(fee_rate, DEFAULT_DISCARD_FEE_RATE);
            match tx {
                Some(t) => {
                    let inputs = &t.input;
                    let outputs = &t.output;
                    assert!(!inputs.is_empty());
                    assert!(outputs.len() == 1 || outputs.len() == 2);
                    let tx_fee = FeeRate::fee_wu(fee_rate, t.weight()).unwrap();

                    let input_sum = calc_input_sum(inputs, &coins);
                    let output_sum: Amount = outputs.iter().map(|o| o.value).sum();
                    let recipient_output = outputs[0].clone();
                    assert_eq!(recipient_output.value, target);
                    assert_eq!(input_sum, output_sum + tx_fee);

                    let amounts = input_amounts(inputs, &coins);
                    let effective_sum: Amount = amounts
                        .iter()
                        .filter_map(|amt| effective_value(FeeRate::ZERO, Weight::from_wu(66), *amt))
                        .map(|sa| sa.to_unsigned().unwrap())
                        .sum();

                    if effective_sum > target + cost_of_change || target == Amount::ZERO {
                        // SRD solution
                        let change_output = outputs[1].clone();
                        assert_eq!(
                            change_output.value,
                            input_sum - tx_fee - recipient_output.value
                        );
                        assert!(outputs.len() == 2);
                    } else {
                        // BNB solution
                        assert!(input_sum + tx_fee >= target);
                        assert!(input_sum < target + cost_of_change);
                        assert!(outputs.len() == 1);
                    }
                }
                None => {
                    let base_tx_fee = calculate_tx_fee(fee_rate, &[], &[&recipient, &change_addr]);
                    if let Some(fee) = base_tx_fee {
                        let total_target = target + fee;
                        let srd_result = single_random_draw(total_target, fee_rate, &coins);
                        assert!(srd_result.is_none());
                    }
                }
            }
            Ok(())
        });
    }
}
