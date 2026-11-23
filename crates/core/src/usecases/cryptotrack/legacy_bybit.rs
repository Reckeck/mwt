//! Scenario of storing exchange records as operations of the module (spec
//! E7), and the mark that tells whether the module reads an exchange wallet. The
//! rules (the account, the coins, the day prices, how a record is merged) are in
//! `cryptotrack::service::legacy_bybit`.

use crate::cryptotrack::service::legacy_bybit::{self as rules, BybitStored};
use crate::error::Result;
use crate::model::store::StoreKind;
use crate::model::vault::{PendingImport, VaultDoc};
use crate::money::tree::{find_store, root_of};
use crate::service::UnlockedVault;

/// Store exchange records as operations of the module. The one way in:
/// both a fresh sweep and the conversion of the old queue come through
/// here, so neither can write an operation the other would not.
///
/// Does not save; the caller does, once.
pub(crate) fn bybit_store(
    vault: &mut UnlockedVault,
    store_id: &str,
    records: &[PendingImport],
) -> Result<BybitStored> {
    let decided = crate::cryptotrack::exchange_sync::decided_import_ids(&vault.doc);
    let root = root_of(&vault.doc, store_id);
    rules::store_records(&mut vault.doc, &decided, &root, records)
}

/// Whether a wallet is read by the crypto module rather than kept by
/// hand: an exchange with a key or with a followed account of its own.
///
/// Spec E11 archives the old hand-kept crypto wallets when the user moves
/// to level 3; a wallet the engine drives must stay, because its
/// operations are refetched and its balance is recomputed. This is the
/// mark E11 reads.
pub fn exchange_is_engine_driven(doc: &VaultDoc, store_id: &str) -> bool {
    let root = root_of(doc, store_id);
    let Some(store) = find_store(doc, &root) else {
        return false;
    };
    if store.kind != StoreKind::Exchange {
        return false;
    }
    let followed = store
        .crypto_account_id
        .as_deref()
        .is_some_and(|id| doc.crypto_accounts.iter().any(|account| account.id == id));
    followed
        || doc
            .exchanges
            .bybit
            .iter()
            .any(|keys| root_of(doc, &keys.store_id) == root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::store::{ExchangeProfile, ExchangeProvider, Store};

    #[test]
    fn a_wallet_is_driven_by_a_followed_account_or_a_key_and_only_if_it_is_an_exchange() {
        let mut doc = VaultDoc::default();
        let mut wallet = Store::new("Bybit", StoreKind::Exchange, "#000");
        wallet.exchange = Some(ExchangeProfile {
            provider: ExchangeProvider::Bybit,
            ..ExchangeProfile::default()
        });
        let wallet_id = wallet.id.clone();
        doc.stores.push(wallet);
        let plain = Store::new("Bank", StoreKind::Bank, "#000");
        let plain_id = plain.id.clone();
        doc.stores.push(plain);
        assert!(!exchange_is_engine_driven(&doc, &wallet_id));

        let account_id = crate::cryptotrack::sources::bybit::account_id_for(&wallet_id);
        doc.stores[0].crypto_account_id = Some(account_id.clone());
        assert!(
            !exchange_is_engine_driven(&doc, &wallet_id),
            "a mark without its account is not a followed account"
        );
        doc.crypto_accounts
            .push(crate::model::cryptotrack::CryptoAccount::new(
                &account_id,
                "Bybit",
                crate::cryptotrack::sources::bybit::EXCHANGE_CHAIN,
                "handle",
                "2023-11-14T00:00:00Z",
            ));
        assert!(exchange_is_engine_driven(&doc, &wallet_id));
        assert!(!exchange_is_engine_driven(&doc, &plain_id));
    }
}
