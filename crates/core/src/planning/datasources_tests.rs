//! Tests of the datasources area: the old inline tests (names and bodies unchanged).

use super::datasources::*;
use crate::model::datasource::Extracted;
use crate::model::datasource::{DataSource, TableRow};
use crate::model::datasource::{
    NumberFormat, SourceInterval, SourceMethod, SourceOutput, SourceStatus,
};
use crate::model::inflation::InflationMode;
use crate::service::test_support::{amount, empty_vault as vault};

fn draft(uses: Vec<SourceUse>) -> DataSourceDraft {
    DataSourceDraft {
        id: None,
        name: "ЦБ".into(),
        url: "https://cbr.ru/hd_base/infl/".into(),
        method: SourceMethod::Table {
            table_selector: "table.data".into(),
            column_header: "Инфляция".into(),
            row: TableRow::ByDate,
        },
        output: SourceOutput::Number,
        number: NumberFormat::default(),
        interval: SourceInterval::Day,
        uses,
    }
}

fn number(value: &str) -> std::result::Result<Extracted, String> {
    Ok(Extracted {
        number: Some(amount(value)),
        ..Extracted::default()
    })
}

#[test]
fn three_failures_in_a_row_break_a_source() {
    let (_dir, mut vault) = vault();
    let id = vault.save_data_source(draft(vec![])).unwrap().source.id;
    for attempt in 1..=2 {
        let notices = vault
            .apply_source_fetch(&id, Err("no table".into()), true)
            .unwrap();
        assert!(notices.is_empty(), "attempt {attempt}");
    }
    let notices = vault
        .apply_source_fetch(&id, Err("no table".into()), true)
        .unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(vault.data_source(&id).unwrap().status, SourceStatus::Broken);
    // A fourth failure does not announce it again; a value repairs it.
    assert!(
        vault
            .apply_source_fetch(&id, Err("no table".into()), true)
            .unwrap()
            .is_empty()
    );
    vault.apply_source_fetch(&id, number("6.35"), true).unwrap();
    let source = vault.data_source(&id).unwrap();
    assert_eq!(source.status, SourceStatus::Ok);
    assert_eq!(source.failures, 0);
    assert_eq!(source.last.as_ref().unwrap().number, Some(amount("6.35")));
}

#[test]
fn an_out_of_range_fetch_is_recorded_and_not_due() {
    let (_dir, mut vault) = vault();
    let id = vault.save_data_source(draft(vec![])).unwrap().source.id;
    vault
        .apply_source_fetch(&id, Err("number out of range".into()), true)
        .unwrap();
    let source = vault.data_source(&id).unwrap();
    assert!(source.checked_at.is_some());
    assert_eq!(source.failures, 1);
    assert!(!is_due(source, chrono::Utc::now()));
}

#[test]
fn a_jump_waits_for_confirmation() {
    let (_dir, mut vault) = vault();
    let id = vault.save_data_source(draft(vec![])).unwrap().source.id;
    vault.apply_source_fetch(&id, number("6.35"), true).unwrap();
    let notices = vault.apply_source_fetch(&id, number("635"), true).unwrap();
    assert_eq!(notices.len(), 1);
    let source = vault.data_source(&id).unwrap();
    assert_eq!(source.status, SourceStatus::Pending);
    assert_eq!(source.last.as_ref().unwrap().number, Some(amount("6.35")));
    vault.confirm_source_value(&id, true, true).unwrap();
    let source = vault.data_source(&id).unwrap();
    assert_eq!(source.status, SourceStatus::Ok);
    assert_eq!(source.last.as_ref().unwrap().number, Some(amount("635")));
    assert_eq!(source.history.len(), 1);
    // A small move is taken at once.
    vault.apply_source_fetch(&id, number("640"), true).unwrap();
    assert_eq!(vault.data_source(&id).unwrap().status, SourceStatus::Ok);
}

#[test]
fn inflation_in_source_mode_reads_the_source() {
    let (_dir, mut vault) = vault();
    let view = vault
        .save_data_source(draft(vec![SourceUse::Inflation {
            currency: "THB".into(),
        }]))
        .unwrap();
    let id = view.source.id;
    assert_eq!(
        view.used_by,
        vec![SourceUse::Inflation {
            currency: "thb".into()
        }]
    );
    assert!(vault.inflation_series("thb").is_empty());
    vault
        .apply_source_fetch(
            &id,
            Ok(Extracted {
                series: vec![
                    crate::model::datasource::SeriesPoint {
                        label: "08.2026".into(),
                        value: amount("1.10"),
                    },
                    crate::model::datasource::SeriesPoint {
                        label: "09.2026".into(),
                        value: amount("1.25"),
                    },
                ],
                ..Extracted::default()
            }),
            true,
        )
        .unwrap();
    let series = vault.inflation_series("thb");
    assert_eq!(series.len(), 2);
    assert_eq!(series[1].month, "2026-09");
    assert_eq!(series[1].yoy_pct, amount("1.25"));
    assert_eq!(vault.inflation_mode("thb"), InflationMode::Source);
    // Deleting the source returns the currency to its default mode.
    vault.delete_data_source(&id).unwrap();
    assert!(vault.doc.settings.inflation_source.is_empty());
    assert_ne!(vault.inflation_mode("thb"), InflationMode::Source);
}

#[test]
fn a_price_source_writes_custom_points_in_usd() {
    let (_dir, mut vault) = vault();
    let id = vault
        .save_data_source(draft(vec![SourceUse::AssetPrice {
            asset_key: "asset:gold".into(),
            currency: "usd".into(),
        }]))
        .unwrap()
        .source
        .id;
    vault.apply_source_fetch(&id, number("2400"), true).unwrap();
    let point = vault
        .doc
        .price_cache
        .iter()
        .find(|point| point.asset_key == "asset:gold")
        .unwrap();
    assert_eq!(point.value_usd, amount("2400"));
    assert_eq!(point.source, format!("custom:{id}"));
}

#[test]
fn a_draft_is_checked() {
    let (_dir, mut vault) = vault();
    let mut bad = draft(vec![]);
    bad.url = "ftp://x".into();
    assert!(vault.save_data_source(bad).is_err());
    let mut text = draft(vec![SourceUse::AssetPrice {
        asset_key: "asset:gold".into(),
        currency: "usd".into(),
    }]);
    text.output = SourceOutput::Text;
    assert!(vault.save_data_source(text).is_err());
}

#[test]
fn the_list_is_sorted_by_name_and_a_fresh_source_is_due() {
    let (_dir, mut vault) = vault();
    let mut second = draft(vec![]);
    second.name = "Alpha".into();
    vault.save_data_source(draft(vec![])).unwrap();
    vault.save_data_source(second).unwrap();
    let names: Vec<String> = vault
        .data_sources_list()
        .into_iter()
        .map(|view| view.source.name)
        .collect();
    assert_eq!(names, ["Alpha", "ЦБ"]);
    let now = chrono::Utc::now();
    assert_eq!(vault.due_data_sources(now).len(), 2);
    assert_eq!(due_data_sources(&vault.doc, now).len(), 2);
}

#[test]
fn a_fetch_records_the_value_and_the_price_it_sets_together() {
    let (_dir, mut vault) = vault();
    let id = vault
        .save_data_source(draft(vec![SourceUse::AssetPrice {
            asset_key: "asset:gold".into(),
            currency: "usd".into(),
        }]))
        .unwrap()
        .source
        .id;
    vault.apply_source_fetch(&id, number("2400"), true).unwrap();
    assert_eq!(
        vault
            .data_source(&id)
            .unwrap()
            .last
            .as_ref()
            .unwrap()
            .number,
        Some(amount("2400"))
    );
    assert!(
        vault
            .doc
            .price_cache
            .iter()
            .any(|point| point.asset_key == "asset:gold")
    );
}

#[test]
fn the_source_delegates_equal_the_scenarios() {
    use crate::usecases::planning::datasources as scenario;
    let (_a, mut by_method) = vault();
    let (_b, mut by_scenario) = vault();
    let method = by_method.save_data_source(draft(vec![])).unwrap().source;
    let made = scenario::save_data_source(&mut by_scenario, draft(vec![]))
        .unwrap()
        .source;
    let shape = |source: &DataSource| (source.name.clone(), source.url.clone(), source.status);
    assert_eq!(shape(&method), shape(&made));
    let method_notices = by_method
        .apply_source_fetch(&method.id, number("6.35"), true)
        .unwrap();
    let made_notices =
        scenario::apply_source_fetch(&mut by_scenario, &made.id, number("6.35"), true).unwrap();
    assert_eq!(method_notices, made_notices);
    // A jump waits; confirming it is the same through both doors.
    by_method
        .apply_source_fetch(&method.id, number("6350"), true)
        .unwrap();
    scenario::apply_source_fetch(&mut by_scenario, &made.id, number("6350"), true).unwrap();
    assert_eq!(
        by_method.data_source(&method.id).unwrap().status,
        SourceStatus::Pending
    );
    assert_eq!(
        by_method
            .confirm_source_value(&method.id, true, true)
            .unwrap(),
        scenario::confirm_source_value(&mut by_scenario, &made.id, true, true).unwrap()
    );
    assert_eq!(
        by_method
            .data_source(&method.id)
            .unwrap()
            .last
            .as_ref()
            .map(|value| value.number),
        by_scenario
            .data_source(&made.id)
            .unwrap()
            .last
            .as_ref()
            .map(|value| value.number)
    );
    by_method.delete_data_source(&method.id).unwrap();
    scenario::delete_data_source(&mut by_scenario, &made.id).unwrap();
    assert!(by_method.data_sources_list().is_empty());
    assert!(by_scenario.data_sources_list().is_empty());
    assert!(by_method.delete_data_source(&method.id).is_err());
    assert!(scenario::delete_data_source(&mut by_scenario, &made.id).is_err());
}

fn fresh_source() -> DataSource {
    new_source(&draft(vec![]), "s1".into(), "2026-03-01T00:00:00Z".into())
}

#[test]
fn a_fetch_that_jumps_waits_and_a_small_move_is_taken() {
    let mut source = fresh_source();
    let at = "2026-03-02T00:00:00Z";
    let first = record_fetch(&mut source, number("6.35"), at, true);
    assert!(first.spread && first.notices.is_empty());
    let small = record_fetch(&mut source, number("6.40"), at, true);
    assert!(small.spread);
    assert_eq!(source.history.len(), 1);
    let jump = record_fetch(&mut source, number("640"), at, true);
    assert!(!jump.spread);
    assert_eq!(jump.notices.len(), 1);
    assert_eq!(source.status, SourceStatus::Pending);
    assert!(source.pending.is_some());
    assert!(!settle_pending(&mut source, false).unwrap());
    assert_eq!(source.status, SourceStatus::Ok);
    assert!(settle_pending(&mut source, true).is_err());
}

#[test]
fn a_value_is_not_spread_without_a_rate_or_a_number() {
    use crate::model::datasource::AssetPriceSource;
    let mut doc = crate::model::vault::VaultDoc::default();
    let mut source = fresh_source();
    record_fetch(&mut source, number("100"), "2026-03-02T00:00:00Z", true);
    doc.asset_price_sources.push(AssetPriceSource {
        asset_key: "crypto:btc".into(),
        source_id: "s1".into(),
        currency: "rub".into(),
    });
    let spread = spread_value(&doc, &source, |_| Some(amount("0")), true, "now");
    assert!(spread.points.is_empty(), "a zero per_usd sets no price");
    let spread = spread_value(&doc, &source, |_| None, true, "now");
    assert!(spread.points.is_empty());
    let spread = spread_value(&doc, &source, |_| Some(amount("50")), true, "now");
    assert_eq!(spread.points.len(), 1);
    assert_eq!(spread.points[0].value_usd, amount("2"));
    let bare = fresh_source();
    assert!(
        spread_value(&doc, &bare, |_| Some(amount("50")), true, "now")
            .points
            .is_empty()
    );
}
