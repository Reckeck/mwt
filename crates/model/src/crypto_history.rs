//! The crypto module's capital history as a plain value: what it was worth at
//! the cutoffs of a report, and which replayed wallets that record speaks for.
//! The cryptotrack engine builds it; the reports read it through the books.

use std::collections::HashSet;

use mwt_base::Amount;

use crate::store::AssetRef;

/// The module's own capital history: what it was worth at the cutoffs of a
/// report, and which replayed wallets that record speaks for.
#[derive(Debug, Default)]
pub struct CryptoHistory {
    /// USD at each cutoff, in the order they were given. `None` is a month the
    /// module has no record of — a gap in tracking, never a zero.
    pub values_usd: Vec<Option<Amount>>,
    /// Archived hand-kept wallets whose coins a followed address now holds.
    pub retired: HashSet<String>,
    /// What the engine accounts for today, to recognize a replayed position of
    /// one of those wallets.
    pub engine_assets: Vec<AssetRef>,
}

impl CryptoHistory {
    /// A history with no record at any of `cutoffs` cutoffs.
    pub fn new(cutoffs: usize) -> Self {
        Self {
            values_usd: vec![None; cutoffs],
            retired: HashSet::new(),
            engine_assets: Vec::new(),
        }
    }

    /// The module's value at the `index`-th cutoff, in USD; `None` for a month
    /// it has no record of.
    pub fn value_usd(&self, index: usize) -> Option<Amount> {
        self.values_usd.get(index).copied().flatten()
    }

    /// Whether a replayed position is the same coins the module's record
    /// already counts: the past half of the rule in `apply_crypto_holdings`.
    /// The caller asks only for a point that has a record.
    pub fn supersedes(&self, store_id: &str, asset: &AssetRef) -> bool {
        self.retired.contains(store_id)
            && self.engine_assets.iter().any(|known| known.same_as(asset))
    }
}
