use std::collections::HashMap;
use std::io::Write;

use rust_decimal::Decimal;

use crate::types::{Account, ClientId, OutputRow, Transaction, TxId};

struct TxRecord {
    client: ClientId,
    amount: Decimal,
    disputed: bool,
}

#[derive(Default)]
pub struct Engine {
    accounts: HashMap<ClientId, Account>,
    records: HashMap<TxId, TxRecord>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, tx: Transaction) -> anyhow::Result<()> {
        let client = match &tx {
            Transaction::Deposit { client, .. }
            | Transaction::Withdrawal { client, .. }
            | Transaction::Dispute { client, .. }
            | Transaction::Resolve { client, .. }
            | Transaction::Chargeback { client, .. } => *client,
        };
        let account = self.accounts.entry(client).or_default();
        // Locked accounts reject every transaction type
        if account.locked {
            return Ok(());
        }

        match tx {
            Transaction::Deposit { tx, amount, .. } => {
                deposit(account, &mut self.records, client, tx, amount)
            }
            Transaction::Withdrawal { tx, amount, .. } => {
                withdrawal(account, &mut self.records, client, tx, amount)
            }
            Transaction::Dispute { tx, .. } => dispute(account, &mut self.records, client, tx),
            Transaction::Resolve { tx, .. } => resolve(account, &mut self.records, client, tx),
            Transaction::Chargeback { tx, .. } => {
                chargeback(account, &mut self.records, client, tx)
            }
        }
    }

    pub fn write_csv<W: Write>(&self, writer: W) -> anyhow::Result<()> {
        let mut csv_writer = csv::Writer::from_writer(writer);
        for (&client, account) in &self.accounts {
            csv_writer.serialize(OutputRow {
                client,
                available: account.available,
                held: account.held,
                total: account.total(),
                locked: account.locked,
            })?;
        }
        csv_writer.flush()?;
        Ok(())
    }
}

fn deposit(
    account: &mut Account,
    records: &mut HashMap<TxId, TxRecord>,
    client: ClientId,
    tx: TxId,
    amount: Decimal,
) -> anyhow::Result<()> {
    account.available = checked(account.available.checked_add(amount), tx)?;
    records.insert(
        tx,
        TxRecord {
            client,
            amount,
            disputed: false,
        },
    );
    Ok(())
}

fn withdrawal(
    account: &mut Account,
    records: &mut HashMap<TxId, TxRecord>,
    client: ClientId,
    tx: TxId,
    amount: Decimal,
) -> anyhow::Result<()> {
    // Insufficient funds: silently no-op
    if account.available >= amount {
        account.available = checked(account.available.checked_sub(amount), tx)?;
        records.insert(
            tx,
            TxRecord {
                client,
                amount,
                disputed: false,
            },
        );
    }
    Ok(())
}

fn dispute(
    account: &mut Account,
    records: &mut HashMap<TxId, TxRecord>,
    client: ClientId,
    tx: TxId,
) -> anyhow::Result<()> {
    if let Some(record) = records.get_mut(&tx)
        && record.client == client
        && !record.disputed
    {
        account.available = checked(account.available.checked_sub(record.amount), tx)?;
        account.held = checked(account.held.checked_add(record.amount), tx)?;
        record.disputed = true;
    }
    Ok(())
}

fn resolve(
    account: &mut Account,
    records: &mut HashMap<TxId, TxRecord>,
    client: ClientId,
    tx: TxId,
) -> anyhow::Result<()> {
    if let Some(record) = records.get_mut(&tx)
        && record.client == client
        && record.disputed
    {
        account.held = checked(account.held.checked_sub(record.amount), tx)?;
        account.available = checked(account.available.checked_add(record.amount), tx)?;
        record.disputed = false;
    }
    Ok(())
}

fn chargeback(
    account: &mut Account,
    records: &mut HashMap<TxId, TxRecord>,
    client: ClientId,
    tx: TxId,
) -> anyhow::Result<()> {
    if let Some(record) = records.get_mut(&tx)
        && record.client == client
        && record.disputed
    {
        account.held = checked(account.held.checked_sub(record.amount), tx)?;
        account.locked = true;
    }
    Ok(())
}

/// `Decimal`'s `+`/`-` panic on overflow; using the checked variants
/// everywhere turns that into a normal error instead (practically
/// unreachable with real-world amounts, but provably handled).
fn checked(result: Option<Decimal>, tx: TxId) -> anyhow::Result<Decimal> {
    result.ok_or_else(|| anyhow::anyhow!("decimal overflow applying tx {tx}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(engine: &Engine, client: ClientId) -> &Account {
        engine.accounts.get(&client).expect("account should exist")
    }

    fn dec(unscaled: i64) -> Decimal {
        Decimal::new(unscaled, 0)
    }

    fn frac(unscaled: i64, scale: u32) -> Decimal {
        Decimal::new(unscaled, scale)
    }

    #[test]
    fn deposit_credits_available() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert_eq!(acc.total(), dec(5));
    }

    #[test]
    fn withdrawal_debits_available() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec(2),
            })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(3));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_noop() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec(100),
            })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(5));
    }

    #[test]
    fn dispute_then_resolve_returns_funds() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(0));
        assert_eq!(account(&engine, 1).held, dec(5));

        engine
            .apply(Transaction::Resolve { client: 1, tx: 1 })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(5));
        assert_eq!(account(&engine, 1).held, dec(0));
    }

    #[test]
    fn dispute_then_chargeback_locks_account() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 1 })
            .unwrap();

        let acc = account(&engine, 1);
        assert_eq!(acc.held, dec(0));
        assert_eq!(acc.total(), dec(0));
        assert!(acc.locked);
    }

    #[test]
    fn dispute_on_unknown_tx_is_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 999 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert_eq!(acc.held, dec(0));
    }

    #[test]
    fn locked_account_rejects_further_transactions() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 1 })
            .unwrap();

        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 2,
                amount: dec(100),
            })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(0));
    }

    #[test]
    fn withdrawal_of_exact_available_balance_succeeds() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec(5),
            })
            .unwrap();
        assert_eq!(account(&engine, 1).available, dec(0));
    }

    #[test]
    fn resolve_on_never_disputed_tx_is_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Resolve { client: 1, tx: 1 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert_eq!(acc.held, dec(0));
    }

    #[test]
    fn chargeback_on_never_disputed_tx_is_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 1 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert!(!acc.locked);
    }

    #[test]
    fn resolve_and_chargeback_on_unknown_tx_are_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Resolve { client: 1, tx: 999 })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 999 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert!(!acc.locked);
    }

    #[test]
    fn disputing_an_already_disputed_tx_is_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        // Second dispute on the same tx must not double-apply the shift.
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(0));
        assert_eq!(acc.held, dec(5));
    }

    #[test]
    fn dispute_referencing_another_clients_tx_is_ignored() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        // tx 1 belongs to client 1, not client 2.
        engine
            .apply(Transaction::Dispute { client: 2, tx: 1 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert_eq!(acc.held, dec(0));
    }

    #[test]
    fn withdrawal_can_be_disputed_and_charged_back() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(10),
            })
            .unwrap();
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 2,
                amount: dec(4),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 2 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(2));
        assert_eq!(acc.held, dec(4));

        engine
            .apply(Transaction::Chargeback { client: 1, tx: 2 })
            .unwrap();
        let acc = account(&engine, 1);
        assert_eq!(acc.held, dec(0));
        assert_eq!(acc.total(), dec(2));
        assert!(acc.locked);
    }

    #[test]
    fn locked_account_rejects_withdrawal_and_dispute_too() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 2,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 1 })
            .unwrap();

        // Account is now locked: neither of these should take effect.
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 3,
                amount: dec(1),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 2 })
            .unwrap();

        let acc = account(&engine, 1);
        assert_eq!(acc.available, dec(5));
        assert_eq!(acc.held, dec(0));
    }

    #[test]
    fn clients_are_independent() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: dec(5),
            })
            .unwrap();
        engine
            .apply(Transaction::Deposit {
                client: 2,
                tx: 2,
                amount: dec(20),
            })
            .unwrap();
        engine
            .apply(Transaction::Dispute { client: 1, tx: 1 })
            .unwrap();
        engine
            .apply(Transaction::Chargeback { client: 1, tx: 1 })
            .unwrap();

        assert!(account(&engine, 1).locked);
        let other = account(&engine, 2);
        assert!(!other.locked);
        assert_eq!(other.available, dec(20));
    }

    #[test]
    fn decimal_precision_round_trips_exactly() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: frac(12345, 4), // 1.2345
            })
            .unwrap();
        engine
            .apply(Transaction::Withdrawal {
                client: 1,
                tx: 2,
                amount: frac(1, 4), // 0.0001
            })
            .unwrap();
        assert_eq!(account(&engine, 1).available, frac(12344, 4)); // 1.2344
    }

    #[test]
    fn deposit_overflow_is_an_error() {
        let mut engine = Engine::new();
        engine
            .apply(Transaction::Deposit {
                client: 1,
                tx: 1,
                amount: Decimal::MAX,
            })
            .unwrap();
        let result = engine.apply(Transaction::Deposit {
            client: 1,
            tx: 2,
            amount: Decimal::ONE,
        });
        assert!(result.is_err());
    }
}
