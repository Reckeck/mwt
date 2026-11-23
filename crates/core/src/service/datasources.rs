//! Shim of the `datasources` area: the rules moved to `crate::planning::datasources`, the
//! scenarios to `crate::usecases::planning::datasources` (spec core-strangler, D1).

pub use crate::planning::datasources::{DataSourceDraft, DataSourceView, SourceUse, is_due};

use crate::datasource::Extracted;
use crate::error::Result;
use crate::model::datasource::DataSource;
use crate::model::notify::NoticeRecord;

use super::UnlockedVault;
use super::forward::forward;

impl UnlockedVault {
    // forward:begin
    forward! {
        pub fn data_source(&self, id: &str) -> Option<&DataSource>
            => crate::planning::datasources::data_source @doc;
        /// Every place that reads the source.
        pub fn source_uses(&self, id: &str) -> Vec<SourceUse>
            => crate::usecases::planning::datasources::source_uses @doc;
        pub fn data_sources_list(&self) -> Vec<DataSourceView>
            => crate::usecases::planning::datasources::data_sources_list @doc;
        /// Create or change a source. A changed address or method starts the
        /// count of failures over; the history stays.
        pub fn save_data_source(&mut self, draft: DataSourceDraft) -> Result<DataSourceView>
            => crate::usecases::planning::datasources::save_data_source;
        /// Delete a source and every use of it; inflation in source mode goes
        /// back to its default.
        pub fn delete_data_source(&mut self, id: &str) -> Result<()>
            => crate::usecases::planning::datasources::delete_data_source;
        /// Sources whose interval has passed since the last attempt.
        pub fn due_data_sources(&self, now: chrono::DateTime<chrono::Utc>) -> Vec<DataSource>
            => crate::planning::datasources::due_data_sources @doc;
        /// Record a fetch. An error keeps the value and counts a failure
        /// (`BROKEN_AFTER` in a row mark the source broken, with a notice); a
        /// number that jumped `JUMP_FACTOR` times waits for confirmation; any
        /// other value becomes the latest and goes to its uses.
        pub fn apply_source_fetch(&mut self, id: &str, fetched: std::result::Result<Extracted, String>, ru: bool) -> Result<Vec<NoticeRecord>>
            => crate::usecases::planning::datasources::apply_source_fetch;
        /// Take or drop the value waiting after a jump.
        pub fn confirm_source_value(&mut self, id: &str, accept: bool, ru: bool) -> Result<Vec<NoticeRecord>>
            => crate::usecases::planning::datasources::confirm_source_value;
    }
    // forward:end

    // Called by the peer-sync repair until that area moves; not in the baseline (crate-private),
    // so outside the regenerated block.
    forward! {}
}
