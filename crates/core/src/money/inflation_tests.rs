//! Tests of the inflation area: the old inline tests (names kept), which drive
//! the same rules through the `UnlockedVault` delegates.

use super::inflation::*;

#[test]
fn the_shipped_table_is_the_one_of_the_app_config() {
    assert_eq!(shipped_inflation(), crate::appconfig::default_inflation());
}
