//! # Transaction Builder Coin
//!
//! A Coin is a possible input for a new Transaction.
//!
//! Each coin is composed of an output (TxOut) and outpoint (OutPoint).  The output contains the
//! value added and the conditions to spend it, while the outpoint is the output location used
//! to create the new transaction input (TxIn).

use bitcoin::{Amount, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Weight, Witness};
use bitcoin_coin_selection::WeightedUtxo;

/// Coin used as source of Transaction Input.
#[derive(Clone, Debug)]
pub struct Coin {
    /// The pointer to location of the output to spend.
    pub outpoint: OutPoint,
    /// The output used to source funds.
    pub tx_out: TxOut,
}

impl Coin {
    pub fn into_input(&self) -> TxIn {
        TxIn {
            previous_output: self.outpoint,
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::default(),
        }
    }
}

impl WeightedUtxo for Coin {
    /// The satisfaction weight of the coin for use by coin-selection.
    fn satisfaction_weight(&self) -> Weight {
        // see rust-bitcoin InputWeightPrediction P2TR_KEY_DEFAULT_SIGHASH
        // for full calculation, see InputWeightPrediction::from_slice()
        // 1 witness_len + 1 item len +  64 signature
        Weight::from_wu(66)
    }

    /// The value of the coin for use by coin-selection.
    fn value(&self) -> Amount {
        self.tx_out.value
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::valued_coin;

    #[test]
    fn to_input() {
        let coin = valued_coin(42, 0);
        let input = coin.into_input();
        assert_eq!(input.previous_output, coin.outpoint);
    }
}
