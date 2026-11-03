//! Corrections ("Adjustment"): set the balances of one account to what is
//! really there. The ledger balance before it is worked out at the
//! correction's own moment, so a back-dated one corrects the past, and it is
//! worked out again whenever the history changes: what a correction keeps is
//! the real balance (`after`), the difference follows the ledger.

use std::collections::HashMap;

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::AdjustmentDraftInput;
use crate::model::store::AssetRef;
use crate::model::transaction::{AdjustmentLine, PriceStamp, Transaction};
use crate::model::vault::now_iso;
use crate::pricing::history::secs_or_now;
use crate::sync::stamp::Stamp;
use crate::sync::view::full_key;

use super::UnlockedVault;

/// What the correction screen opens with.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AdjustmentDraft {
    pub store_id: String,
    /// RFC3339 moment the balances are for.
    pub at: String,
    /// Every asset the account holds then; `after` starts equal to `before`.
    pub lines: Vec<AdjustmentLine>,
}

impl UnlockedVault {
    /// Balance of every asset of one store at `at` (inclusive), leaving out the
    /// transaction `exclude`. Zero balances are dropped.
    pub fn balances_at(
        &self,
        store_id: &str,
        at: &str,
        exclude: Option<&str>,
    ) -> Vec<(AssetRef, Amount)> {
        self.balances_where(store_id, secs_or_now(at), |tx| {
            exclude.is_some_and(|id| tx.meta().id == id)
        })
    }

    /// Balance of every asset of one store up to `limit` seconds (inclusive),
    /// leaving out the transactions `skip` picks. Zero balances are dropped.
    fn balances_where(
        &self,
        store_id: &str,
        limit: i64,
        skip: impl Fn(&Transaction) -> bool,
    ) -> Vec<(AssetRef, Amount)> {
        let mut order: Vec<String> = Vec::new();
        let mut assets: HashMap<String, AssetRef> = HashMap::new();
        let mut totals: HashMap<String, Amount> = HashMap::new();
        for tx in &self.doc.transactions {
            if skip(tx) || secs_or_now(&tx.meta().timestamp) > limit {
                continue;
            }
            for leg in tx.legs() {
                if leg.store_id != store_id || leg.asset.is_derivative() {
                    continue;
                }
                let key = leg.asset.key();
                if !assets.contains_key(&key) {
                    order.push(key.clone());
                    assets.insert(key.clone(), leg.asset.clone());
                }
                *totals.entry(key).or_default() += leg.amount;
            }
        }
        order
            .into_iter()
            .filter_map(|key| {
                let total = totals.get(&key).copied().unwrap_or_default();
                (!total.is_zero()).then(|| (assets.remove(&key).unwrap_or_default(), total))
            })
            .collect()
    }

    /// The correction screen of one account: its assets and their ledger
    /// balances at the moment asked (now by default).
    pub fn adjustment_draft(&self, input: &AdjustmentDraftInput) -> Result<AdjustmentDraft> {
        self.live_store(&input.store_id)?;
        let at = input
            .at
            .clone()
            .filter(|raw| !raw.trim().is_empty())
            .unwrap_or_else(now_iso);
        let lines = self
            .balances_at(&input.store_id, &at, input.replaces.as_deref())
            .into_iter()
            .map(|(asset, balance)| AdjustmentLine {
                asset,
                before: balance,
                after: balance,
                price: PriceStamp::default(),
            })
            .collect();
        Ok(AdjustmentDraft {
            store_id: input.store_id.clone(),
            at,
            lines,
        })
    }

    /// Work out again what the ledger held before every correction, oldest
    /// first, so each one still sets the balance it was given after an
    /// earlier operation was added, edited or deleted. A line whose
    /// difference comes to zero stays: it keeps the balance pinned for the
    /// next change. Corrections of one account at the same moment see only
    /// the ones before them in the list, so the result does not flip-flop.
    pub(crate) fn rebase_adjustments(&mut self) {
        // Corrections at one moment go in the order they were created: by the
        // stamp the record got when it was made, which every device holds the
        // same, so the result does not depend on the order of the list after a
        // sync. One not stamped yet was just added and is the newest; without
        // stamps at all the list order decides.
        let mut order: Vec<(i64, bool, Option<Stamp>, usize)> = self
            .doc
            .transactions
            .iter()
            .enumerate()
            .filter(|(_, tx)| matches!(tx, Transaction::Adjustment(_)))
            .map(|(index, tx)| {
                let created = self
                    .doc
                    .sync
                    .stamps
                    .get(&full_key("transactions", &tx.meta().id))
                    .map(|stamps| stamps.created.clone());
                (
                    secs_or_now(&tx.meta().timestamp),
                    created.is_none(),
                    created,
                    index,
                )
            })
            .collect();
        order.sort_unstable();
        let order: Vec<(i64, usize)> = order
            .into_iter()
            .map(|(secs, _, _, index)| (secs, index))
            .collect();
        for (rank, &(secs, index)) in order.iter().enumerate() {
            let Transaction::Adjustment(inner) = &self.doc.transactions[index] else {
                continue;
            };
            let id = inner.meta.id.clone();
            let store_id = inner.store_id.clone();
            let later: Vec<&str> = order[rank + 1..]
                .iter()
                .take_while(|(other, _)| *other == secs)
                .map(|&(_, other)| self.doc.transactions[other].meta().id.as_str())
                .collect();
            let balances: HashMap<String, Amount> = self
                .balances_where(&store_id, secs, |tx| {
                    tx.meta().id == id || later.contains(&tx.meta().id.as_str())
                })
                .into_iter()
                .map(|(asset, balance)| (asset.key(), balance))
                .collect();
            if let Transaction::Adjustment(inner) = &mut self.doc.transactions[index] {
                for line in &mut inner.lines {
                    line.before = balances.get(&line.asset.key()).copied().unwrap_or_default();
                }
            }
        }
    }

    /// Fill in what the ledger held before the correction, from the other
    /// transactions, and drop the assets whose balance already matches.
    pub(crate) fn complete_adjustment(&self, tx: &mut Transaction) -> Result<()> {
        let Transaction::Adjustment(inner) = tx else {
            return Ok(());
        };
        let balances: HashMap<String, Amount> = self
            .balances_at(&inner.store_id, &inner.meta.timestamp, Some(&inner.meta.id))
            .into_iter()
            .map(|(asset, balance)| (asset.key(), balance))
            .collect();
        for line in &mut inner.lines {
            line.before = balances.get(&line.asset.key()).copied().unwrap_or_default();
        }
        if inner.lines.is_empty() {
            return Ok(());
        }
        // Editing a correction keeps its own assets even when they match now:
        // such a line keeps pinning the balance against later changes.
        let own: Vec<String> = self
            .doc
            .transactions
            .iter()
            .find_map(|tx| match tx {
                Transaction::Adjustment(old) if old.meta.id == inner.meta.id => {
                    Some(old.lines.iter().map(|line| line.asset.key()).collect())
                }
                _ => None,
            })
            .unwrap_or_default();
        inner
            .lines
            .retain(|line| !line.delta().is_zero() || own.contains(&line.asset.key()));
        if inner.lines.is_empty() {
            return Err(Error::InvalidInput(
                "nothing to correct: the balances already match".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use crate::model::store::StoreKind;
    use crate::model::transaction::{AdjustmentTx, TxMeta};
    use crate::service::worth::FlowClass;

    fn correction(store_id: &str, lines: &[(AssetRef, &str)], ts: &str) -> Transaction {
        let mut meta = TxMeta::new("Сверка");
        meta.timestamp = ts.into();
        Transaction::Adjustment(AdjustmentTx {
            meta,
            store_id: store_id.into(),
            lines: lines
                .iter()
                .map(|(asset, actual)| AdjustmentLine {
                    asset: asset.clone(),
                    before: Amount::ZERO,
                    after: amount(actual),
                    price: PriceStamp::default(),
                })
                .collect(),
            fee: None,
        })
    }

    #[test]
    fn a_correction_sets_the_balance_and_remembers_what_was_there() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        vault.add_transaction(salary).unwrap();

        let saved = vault
            .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
            .unwrap();
        let Transaction::Adjustment(inner) = &saved else {
            panic!("a correction");
        };
        assert_eq!(inner.lines[0].before, amount("100"));
        assert_eq!(inner.lines[0].delta(), amount("-20"));
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));
    }

    #[test]
    fn a_correction_is_neither_income_nor_spending_nor_a_sale() {
        let (_dir, mut vault) = setup();
        let wallet = vault.doc.stores[1].id.clone();
        let mut buy = income_of(&wallet, btc(), "1", "30000", "bought");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();
        let before = vault.realized_pnl().total_base;

        let tx = vault
            .add_transaction(correction(
                &wallet,
                &[(btc(), "0.5")],
                "2026-02-01T00:00:00Z",
            ))
            .unwrap();
        assert_eq!(vault.classify_flow(&tx), FlowClass::Ignored);
        assert_eq!(vault.realized_pnl().total_base, before, "nothing was sold");
        assert_eq!(holding_of(&vault, &wallet, &btc()), amount("0.5"));
    }

    #[test]
    fn a_back_dated_correction_sees_the_balance_of_its_moment() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        for (sum, ts) in [
            ("100", "2026-03-01T10:00:00Z"),
            ("50", "2026-03-10T10:00:00Z"),
        ] {
            let mut tx = income(&cash, sum, "salary");
            at(&mut tx, ts);
            vault.add_transaction(tx).unwrap();
        }
        let saved = vault
            .add_transaction(correction(&cash, &[(usd(), "90")], "2026-03-05T10:00:00Z"))
            .unwrap();
        let Transaction::Adjustment(inner) = &saved else {
            panic!("a correction");
        };
        assert_eq!(inner.lines[0].before, amount("100"));
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("140"));

        // Edited: it does not see itself as part of the ledger.
        let id = saved.meta().id.clone();
        let edited = vault
            .update_transaction(
                &id,
                correction(&cash, &[(usd(), "95")], "2026-03-05T10:00:00Z"),
            )
            .unwrap();
        let Transaction::Adjustment(inner) = &edited else {
            panic!("a correction");
        };
        assert_eq!(inner.lines[0].before, amount("100"));
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("145"));
    }

    fn before_of(vault: &UnlockedVault, id: &str) -> Vec<Amount> {
        let Some(Transaction::Adjustment(inner)) =
            vault.doc.transactions.iter().find(|tx| tx.meta().id == id)
        else {
            panic!("a correction");
        };
        inner.lines.iter().map(|line| line.before).collect()
    }

    #[test]
    fn a_correction_follows_changes_to_the_history_before_it() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        let salary = vault.add_transaction(salary).unwrap();
        let fixed = vault
            .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
            .unwrap();
        let fixed = fixed.meta().id.clone();
        let mut later = income(&cash, "10", "salary");
        at(&mut later, "2026-03-10T10:00:00Z");
        vault.add_transaction(later).unwrap();
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

        // Added before it: the counted 80 still stands.
        let mut forgotten = payment(&cash, "15", "lunch");
        at(&mut forgotten, "2026-03-03T10:00:00Z");
        let forgotten = vault.add_transaction(forgotten).unwrap();
        assert_eq!(before_of(&vault, &fixed), [amount("85")]);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

        // Edited before it.
        let mut bigger = income(&cash, "120", "salary");
        at(&mut bigger, "2026-03-01T10:00:00Z");
        vault.update_transaction(&salary.meta().id, bigger).unwrap();
        assert_eq!(before_of(&vault, &fixed), [amount("105")]);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));

        // Deleted before it; the difference may come to nothing, the line stays.
        vault.delete_transaction(&forgotten.meta().id).unwrap();
        let mut spent = payment(&cash, "40", "rent");
        at(&mut spent, "2026-03-02T10:00:00Z");
        vault.add_transaction(spent).unwrap();
        assert_eq!(before_of(&vault, &fixed), [amount("80")]);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
    }

    #[test]
    fn corrections_in_a_row_are_recalculated_in_order() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let first = vault
            .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
            .unwrap();
        let mut between = payment(&cash, "10", "lunch");
        at(&mut between, "2026-03-07T10:00:00Z");
        let between = vault.add_transaction(between).unwrap();
        let second = vault
            .add_transaction(correction(&cash, &[(usd(), "60")], "2026-03-09T10:00:00Z"))
            .unwrap();
        assert_eq!(before_of(&vault, &second.meta().id), [amount("70")]);

        // A change before both moves only the first one's difference.
        let mut bonus = income(&cash, "50", "bonus");
        at(&mut bonus, "2026-03-02T10:00:00Z");
        vault.add_transaction(bonus).unwrap();
        assert_eq!(before_of(&vault, &first.meta().id), [amount("150")]);
        assert_eq!(before_of(&vault, &second.meta().id), [amount("70")]);

        // A change between them moves only the second one's.
        vault.delete_transaction(&between.meta().id).unwrap();
        assert_eq!(before_of(&vault, &first.meta().id), [amount("150")]);
        assert_eq!(before_of(&vault, &second.meta().id), [amount("80")]);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("60"));
    }

    #[test]
    fn corrections_at_the_same_moment_settle() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let moment = "2026-03-05T10:00:00Z";
        vault
            .add_transaction(correction(&cash, &[(usd(), "80")], moment))
            .unwrap();
        vault
            .add_transaction(correction(&cash, &[(usd(), "70")], moment))
            .unwrap();
        let held = holding_of(&vault, &cash, &usd());
        vault.rebase_adjustments();
        vault.recompute_holdings();
        assert_eq!(holding_of(&vault, &cash, &usd()), held, "stable");
        assert_eq!(held, amount("70"));
    }

    #[test]
    fn the_draft_lists_what_the_account_holds() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        vault
            .add_transaction(income(&cash, "100", "salary"))
            .unwrap();
        let draft = vault
            .adjustment_draft(&AdjustmentDraftInput {
                store_id: cash.clone(),
                at: None,
                replaces: None,
            })
            .unwrap();
        assert_eq!(draft.lines.len(), 1);
        assert_eq!(draft.lines[0].before, amount("100"));
        assert_eq!(draft.lines[0].after, amount("100"));
    }

    #[test]
    fn a_correction_keeps_to_the_rules_of_the_account() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        vault
            .add_transaction(income(&bank, "100", "salary"))
            .unwrap();
        let now = now_iso();
        assert!(
            vault
                .add_transaction(correction(&bank, &[(btc(), "1")], &now))
                .is_err(),
            "a bank holds only fiat"
        );
        assert!(
            vault
                .add_transaction(correction(&bank, &[(usd(), "-5")], &now))
                .is_err(),
            "money cannot be below zero"
        );
        assert!(
            vault
                .add_transaction(correction(&bank, &[(usd(), "100")], &now))
                .is_err(),
            "nothing to correct"
        );
        assert!(
            vault
                .add_transaction(correction(&bank, &[(usd(), "1"), (usd(), "2")], &now))
                .is_err(),
            "an asset twice"
        );
    }

    #[test]
    fn re_saving_a_correction_keeps_its_assets_even_when_they_match() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:00Z");
        vault.add_transaction(salary).unwrap();
        let saved = vault
            .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"))
            .unwrap();
        let id = saved.meta().id.clone();

        // A later change of history before it: the correction absorbs it.
        let mut refund = income(&cash, "20", "refund");
        at(&mut refund, "2026-03-02T10:00:00Z");
        vault.add_transaction(refund).unwrap();
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));

        // Opened and saved as it is: the line stays, so does the balance.
        let resaved = vault
            .update_transaction(
                &id,
                correction(&cash, &[(usd(), "80")], "2026-03-05T10:00:00Z"),
            )
            .unwrap();
        let Transaction::Adjustment(inner) = &resaved else {
            panic!("a correction");
        };
        assert_eq!(inner.lines.len(), 1);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("80"));

        // A brand-new correction that changes nothing is still refused.
        assert!(
            vault
                .add_transaction(correction(&cash, &[(usd(), "80")], "2026-03-06T10:00:00Z"))
                .is_err()
        );
    }

    #[test]
    fn re_saving_an_operation_unchanged_does_not_move_the_balance() {
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut salary = income(&cash, "100", "salary");
        at(&mut salary, "2026-03-01T10:00:30Z");
        let salary = vault.add_transaction(salary).unwrap();
        vault
            .add_transaction(correction(&cash, &[(usd(), "90")], "2026-03-01T10:00:45Z"))
            .unwrap();
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
        // The same moment, seconds and all: nothing moves past the correction.
        vault
            .update_transaction(&salary.meta().id.clone(), salary.clone())
            .unwrap();
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("90"));
    }
}
