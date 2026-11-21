//! Properties of the time helpers.

use proptest::prelude::*;

use crate::props::day;
use crate::time::{date_secs, date_ts, unix_secs};

proptest! {
    #[test]
    fn date_ts_reads_back_as_date_secs(date in day()) {
        prop_assert_eq!(unix_secs(&date_ts(date)), Some(date_secs(date)));
    }

    #[test]
    fn unix_secs_never_panics(raw in "\\PC{0,40}") {
        let _ = unix_secs(&raw);
    }
}
