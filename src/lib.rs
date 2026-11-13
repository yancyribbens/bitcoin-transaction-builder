//! # Transaction Builder
//!
//! A Transaction builder for Bitcoin Transactions.

pub mod coin;
pub mod p2tr_transaction;

#[cfg(test)]
mod tests {
    use bitcoin::hashes::Hash;
    use bitcoin::{Amount, OutPoint, ScriptBuf, TxOut, Txid};

    use crate::coin::Coin;

    pub fn valued_coin(val: u64, vout: u32) -> Coin {
        let outpoint: OutPoint = OutPoint {
            txid: Txid::from_byte_array([1; 32]), // Not all zeros
            vout,
        };

        let amount = Amount::from_sat(val);
        let tx_out = TxOut { value: amount, script_pubkey: ScriptBuf::new() };
        Coin { outpoint, tx_out }
    }

    pub fn valued_coins(values: &[u64]) -> Vec<Coin> {
        let mut coins = vec![];
        for (i, val) in values.iter().enumerate() {
            let coin = valued_coin(*val, i as u32);
            coins.push(coin);
        }
        coins
    }
}
