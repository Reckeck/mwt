//! The crypto module's capital history as the reports read it (spec core-strangler, D1).

use crate::Amount;
use crate::model::store::AssetRef;
use crate::money::books::Books;

/// What the crypto module was worth at each cutoff in USD (`None`: no record that month) and
/// whether it already counts a replayed wallet's coins.
pub(crate) fn crypto_module<InScope: Fn(&str) -> bool>(
    books: &Books<'_>,
    cutoffs: &[i64],
    in_scope: InScope,
) -> (
    Vec<Option<Amount>>,
    impl Fn(&str, &AssetRef) -> bool + use<InScope>,
) {
    let history = books.crypto_history(cutoffs, &in_scope);
    let values = (0..cutoffs.len())
        .map(|index| history.value_usd(index))
        .collect();
    (values, move |store_id, asset| {
        history.supersedes(store_id, asset)
    })
}
