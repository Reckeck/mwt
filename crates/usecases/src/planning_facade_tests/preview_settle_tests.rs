//! op-period review: a preview settles a draft the way the save does (I1), so a draft the save
//! accepts does not preview as refused.

use super::period_rule_tests::{quarter, salary, vault_with_card, with_spread};
use crate::usecases::money::transactions::{add_transaction, settle_for_preview};
use mwt_model::SpreadMode;

fn preview_error(
    vault: &crate::service::Session,
    mut tx: crate::model::Transaction,
) -> Option<String> {
    settle_for_preview(vault, &mut tx, None, false);
    mwt_money::preview::preview_transaction(&vault.books(), tx, None).error
}

#[test]
fn an_even_quarter_spread_without_shares_previews_without_error_like_the_save() {
    let (_dir, mut vault, card) = vault_with_card();
    let draft = with_spread(salary(&card, "100.00"), quarter(), SpreadMode::Even, &[]);
    assert_eq!(preview_error(&vault, draft.clone()), None);
    add_transaction(&mut vault, draft).unwrap();
}

#[test]
fn a_bad_manual_spread_previews_with_an_error() {
    let (_dir, vault, card) = vault_with_card();
    let draft = with_spread(
        salary(&card, "100.00"),
        quarter(),
        SpreadMode::Manual,
        &[
            ("2026-10", "10.00"),
            ("2026-11", "10.00"),
            ("2026-12", "10.00"),
        ],
    );
    assert!(preview_error(&vault, draft).is_some());
}
