use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub type ClientId = u16;
pub type TxId = u32;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TxKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Raw CSV row; `amount` is optional since dispute/resolve/chargeback rows omit it.
#[derive(Debug, Deserialize)]
pub struct Record {
    #[serde(rename = "type")]
    pub kind: TxKind,
    pub client: ClientId,
    pub tx: TxId,
    pub amount: Option<Decimal>,
}

/// The transaction shape the engine consumes
pub enum Transaction {
    Deposit {
        client: ClientId,
        tx: TxId,
        amount: Decimal,
    },
    Withdrawal {
        client: ClientId,
        tx: TxId,
        amount: Decimal,
    },
    Dispute {
        client: ClientId,
        tx: TxId,
    },
    Resolve {
        client: ClientId,
        tx: TxId,
    },
    Chargeback {
        client: ClientId,
        tx: TxId,
    },
}

impl TryFrom<Record> for Transaction {
    type Error = anyhow::Error;

    fn try_from(record: Record) -> Result<Self, Self::Error> {
        let Record {
            kind,
            client,
            tx,
            amount,
        } = record;
        let require_amount =
            || amount.ok_or_else(|| anyhow::anyhow!("{kind:?} row for tx {tx} is missing amount"));
        Ok(match kind {
            TxKind::Deposit => Transaction::Deposit {
                client,
                tx,
                amount: require_amount()?,
            },
            TxKind::Withdrawal => Transaction::Withdrawal {
                client,
                tx,
                amount: require_amount()?,
            },
            TxKind::Dispute => Transaction::Dispute { client, tx },
            TxKind::Resolve => Transaction::Resolve { client, tx },
            TxKind::Chargeback => Transaction::Chargeback { client, tx },
        })
    }
}

/// `total` isn't stored — it's derived, so it can never drift from `available + held`.
#[derive(Debug, Default)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

#[derive(Debug, Serialize)]
pub struct OutputRow {
    pub client: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_without_amount_is_an_error() {
        let record = Record {
            kind: TxKind::Deposit,
            client: 1,
            tx: 1,
            amount: None,
        };
        assert!(Transaction::try_from(record).is_err());
    }

    #[test]
    fn withdrawal_without_amount_is_an_error() {
        let record = Record {
            kind: TxKind::Withdrawal,
            client: 1,
            tx: 1,
            amount: None,
        };
        assert!(Transaction::try_from(record).is_err());
    }

    #[test]
    fn dispute_family_rows_convert_without_an_amount() {
        for kind in [TxKind::Dispute, TxKind::Resolve, TxKind::Chargeback] {
            let record = Record {
                kind,
                client: 1,
                tx: 1,
                amount: None,
            };
            assert!(Transaction::try_from(record).is_ok());
        }
    }
}
