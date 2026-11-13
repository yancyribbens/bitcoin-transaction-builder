//! # Transaction Builder Coin
//!
//! A Coin is a possible input for a new Transaction.
//!
//! Each coin is composed of an output (TxOut) and outpoint (OutPoint).  The output contains the
//! value added and the conditions to spend it, while the outpoint is the output location used
//! to create the new transaction input (TxIn).

use bitcoin::{OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Witness};

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
