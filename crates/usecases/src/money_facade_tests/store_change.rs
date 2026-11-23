//! Tests of the store_change area that need the vault (`Session`): moved from
//! `money/store_change_tests.rs` with their assertions unchanged (crate-split T13).

#[cfg(test)]
mod tests {
    use crate::model::chain::Chain;
    use crate::model::store::{AssetRef, Binding, MarketKind, StoreKind, SubType};
    use crate::money::store_change::*;
    use crate::service::Session;
    use crate::service::StoreExtras;
    use crate::service::test_support::*;

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn sub(
        vault: &mut Session,
        parent: &str,
        name: &str,
        sub_type: SubType,
        binding: Binding,
    ) -> String {
        crate::usecases::money::stores::create_store_with(
            vault,
            name,
            StoreKind::Bank,
            "#000",
            "",
            Some(parent.into()),
            StoreExtras {
                sub_type: Some(sub_type),
                binding: Some(binding),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id
    }

    fn change(store_id: &str, change: StoreChange) -> StoreChangeInput {
        StoreChangeInput {
            store_id: store_id.into(),
            change,
        }
    }

    #[test]
    fn a_bank_becomes_a_crypto_wallet_and_its_deposit_is_split_off() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &bank,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        let deposit = sub(
            &mut vault,
            &bank,
            "Deposit",
            SubType::Deposit,
            Binding::Asset(rub()),
        );
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&card, rub(), "100", "0.01", "salary"),
        )
        .unwrap();

        let input = change(
            &bank,
            StoreChange::ChangeKind {
                kind: StoreKind::Crypto,
            },
        );
        let plan = crate::usecases::money::store_change::plan_store_change(&vault, &input);
        assert_eq!(plan.error, None);
        assert!(plan.conflicts.is_empty(), "{:?}", plan.conflicts);
        assert_eq!(plan.kept.len(), 2);
        assert_eq!(plan.kept[1].sub_type_after, Some(SubType::Token));
        assert_eq!(plan.archived.len(), 1);
        assert_eq!(
            crate::money::tree::find_store(&vault.doc, &bank)
                .unwrap()
                .kind,
            StoreKind::Bank,
            "a plan changes nothing"
        );

        crate::usecases::money::store_change::apply_store_change(&mut vault, &input).unwrap();
        assert_eq!(
            crate::money::tree::find_store(&vault.doc, &bank)
                .unwrap()
                .kind,
            StoreKind::Crypto
        );
        let card_after = crate::money::tree::find_store(&vault.doc, &card).unwrap();
        assert_eq!(card_after.kind, StoreKind::Crypto);
        assert_eq!(card_after.sub_type, Some(SubType::Token));
        let split = crate::money::tree::find_store(&vault.doc, &deposit).unwrap();
        assert!(split.archived && split.parent_id.is_none());
        assert_eq!(split.kind, StoreKind::Bank);
        assert_eq!(holding_of(&vault, &card, &rub()), amount("100"));
        assert!(vault.path.with_file_name("test.wallet.bak").exists());
    }

    #[test]
    fn a_wallet_holding_tokens_cannot_become_a_bank() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[2].id.clone();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&cold, btc(), "1", "30000", "bought"),
        )
        .unwrap();
        let input = change(
            &cold,
            StoreChange::ChangeKind {
                kind: StoreKind::Bank,
            },
        );
        let plan = crate::usecases::money::store_change::plan_store_change(&vault, &input);
        assert_eq!(plan.conflicts.len(), 1);
        assert!(
            crate::usecases::money::store_change::apply_store_change(&mut vault, &input).is_err()
        );
        assert_eq!(
            crate::money::tree::find_store(&vault.doc, &cold)
                .unwrap()
                .kind,
            StoreKind::Crypto
        );
    }

    #[test]
    fn a_card_moves_to_another_bank_and_a_token_account_to_a_crypto_wallet() {
        let (_dir, mut vault) = empty_vault();
        let first = store(&mut vault, "First", StoreKind::Bank, None);
        let second = store(&mut vault, "Second", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &first,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &change(
                &card,
                StoreChange::MoveTo {
                    parent_id: second.clone(),
                },
            ),
        )
        .unwrap();
        assert_eq!(
            crate::money::tree::find_store(&vault.doc, &card)
                .unwrap()
                .parent_id
                .as_deref(),
            Some(second.as_str())
        );

        // A card bound to rubles under a crypto wallet becomes a token account.
        let wallet = store(&mut vault, "Wallet", StoreKind::Crypto, None);
        crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &change(
                &card,
                StoreChange::MoveTo {
                    parent_id: wallet.clone(),
                },
            ),
        )
        .unwrap();
        let moved = crate::money::tree::find_store(&vault.doc, &card).unwrap();
        assert_eq!(
            (moved.kind, moved.sub_type),
            (StoreKind::Crypto, Some(SubType::Token))
        );
    }

    #[test]
    fn a_sub_account_becomes_a_wallet_and_a_one_asset_wallet_an_account() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = sub(
            &mut vault,
            &bank,
            "Card",
            SubType::Card,
            Binding::Asset(rub()),
        );
        crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &change(&card, StoreChange::Detach),
        )
        .unwrap();
        let alone = crate::money::tree::find_store(&vault.doc, &card).unwrap();
        assert!(alone.parent_id.is_none() && alone.sub_type.is_none());

        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&card, rub(), "10", "0.01", "salary"),
        )
        .unwrap();
        crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &change(
                &card,
                StoreChange::MoveTo {
                    parent_id: bank.clone(),
                },
            ),
        )
        .unwrap();
        let back = crate::money::tree::find_store(&vault.doc, &card).unwrap();
        assert_eq!(back.sub_type, Some(SubType::Card));
        assert_eq!(back.binding, Binding::Asset(rub()));
    }

    #[test]
    fn merging_moves_every_operation_and_drops_the_account() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let first = sub(
            &mut vault,
            &bank,
            "Card 1",
            SubType::Card,
            Binding::Asset(rub()),
        );
        let second = sub(
            &mut vault,
            &bank,
            "Card 2",
            SubType::Card,
            Binding::Asset(rub()),
        );
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&first, rub(), "100", "0.01", "salary"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&second, rub(), "50", "0.01", "salary"),
        )
        .unwrap();
        let plan = crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &change(
                &first,
                StoreChange::MergeInto {
                    target_id: second.clone(),
                },
            ),
        )
        .unwrap();
        assert_eq!(plan.moved_transactions, 1);
        assert!(crate::money::tree::find_store(&vault.doc, &first).is_none());
        assert_eq!(holding_of(&vault, &second, &rub()), amount("150"));

        // A transfer between the two would land on one account: refused.
        let third = sub(
            &mut vault,
            &bank,
            "Card 3",
            SubType::Card,
            Binding::Asset(rub()),
        );
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            transfer_between(
                crate::model::transaction::AccountRef::Store(second.clone()),
                crate::model::transaction::AccountRef::Store(third.clone()),
                rub(),
                "10",
            ),
        )
        .unwrap();
        let plan = crate::usecases::money::store_change::plan_store_change(
            &vault,
            &change(&third, StoreChange::MergeInto { target_id: second }),
        );
        assert_eq!(plan.conflicts.len(), 1);
    }

    #[test]
    fn retype_maps_one_asset_accounts_between_kinds() {
        let btc_binding = Binding::Asset(AssetRef::crypto("BTC", "Bitcoin", None, Chain::Bitcoin));
        assert_eq!(
            retype(Some(SubType::Token), &btc_binding, StoreKind::Bank),
            None,
            "a bank holds no bitcoin"
        );
        assert_eq!(
            retype(
                Some(SubType::Funding),
                &Binding::Market(MarketKind::Funding),
                StoreKind::Bank
            ),
            None
        );
        assert_eq!(
            retype(
                Some(SubType::Account),
                &Binding::Asset(rub()),
                StoreKind::Bank
            ),
            Some(SubType::Account),
            "an older account stays what it is"
        );
    }
}

#[cfg(test)]
mod merge_correction_tests {
    use crate::Amount;
    use crate::model::store::{AssetRef, Binding, StoreKind, SubType};
    use crate::model::transaction::{
        AdjustmentLine, AdjustmentTx, PriceStamp, Transaction, TxMeta,
    };
    use crate::money::store_change::*;
    use crate::service::Session;
    use crate::service::test_support::*;
    use crate::time::secs_or_now;

    fn rub() -> AssetRef {
        AssetRef::fiat("RUB", "RUB")
    }

    fn card(vault: &mut Session, bank: &str, name: &str) -> String {
        crate::usecases::money::stores::create_store_with(
            vault,
            name,
            StoreKind::Bank,
            "#000",
            "",
            Some(bank.into()),
            crate::service::StoreExtras {
                sub_type: Some(SubType::Card),
                binding: Some(Binding::Asset(rub())),
                ..crate::service::StoreExtras::default()
            },
        )
        .unwrap()
        .id
    }

    fn correct(vault: &mut Session, store_id: &str, after: &str, timestamp: &str) -> String {
        let mut meta = TxMeta::new("count");
        meta.timestamp = timestamp.into();
        let saved = crate::usecases::money::transactions::add_transaction(
            vault,
            Transaction::Adjustment(AdjustmentTx {
                meta,
                store_id: store_id.into(),
                lines: vec![AdjustmentLine {
                    asset: rub(),
                    before: Amount::ZERO,
                    after: amount(after),
                    price: PriceStamp::default(),
                }],
                fee: None,
            }),
        )
        .unwrap();
        saved.meta().id.clone()
    }

    fn topup_at(vault: &mut Session, store_id: &str, raw: &str, timestamp: &str) {
        let mut tx = income_of(store_id, rub(), raw, "0.01", "in");
        at(&mut tx, timestamp);
        crate::usecases::money::transactions::add_transaction(vault, tx).unwrap();
    }

    /// `(before, after)` of the correction line.
    fn line_of(vault: &Session, id: &str) -> (Amount, Amount) {
        vault
            .doc
            .transactions
            .iter()
            .find_map(|tx| match tx {
                Transaction::Adjustment(inner) if inner.meta.id == id => {
                    Some((inner.lines[0].before, inner.lines[0].after))
                }
                _ => None,
            })
            .unwrap()
    }

    fn scenario() -> (tempfile::TempDir, Session, String, String, String, String) {
        let (dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let first = card(&mut vault, &bank, "Card 1");
        let second = card(&mut vault, &bank, "Card 2");
        topup_at(&mut vault, &first, "300", "2026-01-01T00:00:00Z");
        let source_fix = correct(&mut vault, &first, "80", "2026-03-01T00:00:00Z");
        topup_at(&mut vault, &first, "50", "2026-03-10T00:00:00Z");
        topup_at(&mut vault, &second, "1000", "2026-01-15T00:00:00Z");
        let target_fix = correct(&mut vault, &second, "1200", "2026-04-01T00:00:00Z");
        (dir, vault, first, second, source_fix, target_fix)
    }

    fn merge(first: &str, second: &str) -> StoreChangeInput {
        StoreChangeInput {
            store_id: first.into(),
            change: StoreChange::MergeInto {
                target_id: second.into(),
            },
        }
    }

    #[test]
    fn merge_keeps_every_delta_and_the_sum() {
        let (_dir, mut vault, first, second, source_fix, target_fix) = scenario();
        let delta = |vault: &Session, id: &str| {
            let (before, after) = line_of(vault, id);
            after - before
        };
        let source_delta = delta(&vault, &source_fix);
        let target_delta = delta(&vault, &target_fix);
        let sum = holding_of(&vault, &first, &rub()) + holding_of(&vault, &second, &rub());

        let plan = crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &merge(&first, &second),
        )
        .unwrap();

        assert_eq!(plan.corrections.len(), 2);
        assert_eq!(plan.corrections[0].after_old, amount("80"));
        assert_eq!(plan.corrections[0].after_new, amount("1080"));
        assert_eq!(plan.corrections[1].after_old, amount("1200"));
        assert_eq!(plan.corrections[1].after_new, amount("1330"));
        assert_eq!(delta(&vault, &source_fix), source_delta);
        assert_eq!(delta(&vault, &target_fix), target_delta);
        assert_eq!(line_of(&vault, &source_fix).1, amount("1080"));
        assert_eq!(holding_of(&vault, &second, &rub()), sum);
        assert_eq!(sum, amount("1330"));
        let balance_on = |stamp: &str| {
            crate::planning::automations::balance_at(
                &vault.doc,
                &second,
                &rub().key(),
                secs_or_now(stamp),
            )
        };
        assert_eq!(balance_on("2026-03-01T00:00:00Z"), amount("1080"));
        assert_eq!(balance_on("2026-03-10T00:00:00Z"), amount("1130"));
    }

    #[test]
    fn two_cards_correcting_in_the_same_second_keep_their_deltas_and_the_sum() {
        let (_dir, mut vault, first, second, _source_fix, _target_fix) = scenario();
        let stamp = "2026-05-01T00:00:00Z";
        let first_fix = correct(&mut vault, &first, "500", stamp);
        let second_fix = correct(&mut vault, &second, "2000", stamp);
        let delta = |vault: &Session, id: &str| {
            let (before, after) = line_of(vault, id);
            after - before
        };
        let first_delta = delta(&vault, &first_fix);
        let second_delta = delta(&vault, &second_fix);
        let sum = holding_of(&vault, &first, &rub()) + holding_of(&vault, &second, &rub());

        crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &merge(&first, &second),
        )
        .unwrap();

        assert_eq!(delta(&vault, &first_fix), first_delta);
        assert_eq!(delta(&vault, &second_fix), second_delta);
        assert_eq!(holding_of(&vault, &second, &rub()), sum);
    }

    #[test]
    fn the_plan_shows_what_apply_does() {
        let (_dir, mut vault, first, second, source_fix, _target_fix) = scenario();
        let document = serde_json::to_value(&vault.doc).unwrap();
        let planned = crate::usecases::money::store_change::plan_store_change(
            &vault,
            &merge(&first, &second),
        );
        assert_eq!(serde_json::to_value(&vault.doc).unwrap(), document);
        assert_eq!(line_of(&vault, &source_fix).1, amount("80"));

        let applied = crate::usecases::money::store_change::apply_store_change(
            &mut vault,
            &merge(&first, &second),
        )
        .unwrap();
        assert_eq!(planned.corrections, applied.corrections);
    }
}
