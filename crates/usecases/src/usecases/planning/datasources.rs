//! Scenarios of the data sources: saving, deleting, applying what a
//! background fetch brought. Each decides in `planning::datasources` and ends
//! in one `vault.transact`; only this file reads the clock. The prices a
//! source sets are merged into the cache by the same transact that records
//! the fetch (a fetch, then one write), not by a call into the rates area.
//! The uses that read workflow graphs live here: `planning` may not use
//! `workflow`.

use crate::error::Result;
use crate::model::datasource::DataSource;
use crate::model::notify::NoticeRecord;
use crate::model::vault::{VaultDoc, now_iso};
use crate::planning::datasources::{
    self as rules, DataSourceDraft, DataSourceView, SourceUse, Spread,
};
use crate::pricing::history::merge_prices;
use crate::service::Session;
use crate::workflow::graph::Graph;

/// Every place that reads the source.
pub fn source_uses(doc: &VaultDoc, id: &str) -> Vec<SourceUse> {
    let mut uses = rules::linked_uses(doc, id);
    for workflow in &doc.workflows {
        let Ok(graph) = Graph::parse(&workflow.graph) else {
            continue;
        };
        let reads = graph.nodes.iter().any(|node| {
            node.kind.ends_with(".source")
                && node
                    .params
                    .get("source")
                    .and_then(serde_json::Value::as_str)
                    == Some(id)
        });
        if reads {
            uses.push(SourceUse::Workflow {
                workflow_id: workflow.id.clone(),
                name: workflow.name.clone(),
            });
        }
    }
    uses
}

pub fn data_sources_list(doc: &VaultDoc) -> Vec<DataSourceView> {
    let mut out: Vec<DataSourceView> = doc
        .data_sources
        .iter()
        .map(|source| DataSourceView {
            source: source.clone(),
            used_by: source_uses(doc, &source.id),
        })
        .collect();
    out.sort_by_key(|view| view.source.name.to_lowercase());
    out
}

/// Drop the inflation choices that point at a source that no longer
/// exists. Callers persist.
pub fn prune_inflation_sources(vault: &mut Session) {
    rules::prune_inflation_sources(&mut vault.doc);
}

/// Create or change a source. A changed address or method starts the
/// count of failures over; the history stays.
pub fn save_data_source(vault: &mut Session, draft: DataSourceDraft) -> Result<DataSourceView> {
    rules::check_draft(&draft)?;
    let (id, source) = match &draft.id {
        Some(id) => {
            let mut source = rules::data_source(&vault.doc, id)
                .cloned()
                .ok_or_else(|| rules::source_not_found(id))?;
            rules::edit_source(&mut source, &draft);
            (id.clone(), source)
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let source = rules::new_source(&draft, id.clone(), now_iso());
            (id, source)
        }
    };
    let known = draft.id.is_some();
    vault.transact(|tx| {
        if known {
            tx.collection::<DataSource>()
                .update(&id, |slot| *slot = source.clone())?;
        } else {
            tx.collection::<DataSource>().insert(source.clone())?;
        }
        rules::set_source_uses(tx.doc_mut(), &id, &draft.uses);
        Ok(())
    })?;
    Ok(DataSourceView {
        used_by: source_uses(&vault.doc, &id),
        source: rules::data_source(&vault.doc, &id)
            .cloned()
            .unwrap_or_else(|| unreachable!()),
    })
}

/// Delete a source and every use of it; inflation in source mode goes
/// back to its default.
pub fn delete_data_source(vault: &mut Session, id: &str) -> Result<()> {
    vault.transact(|tx| {
        tx.collection::<DataSource>()
            .remove(id)
            .map_err(|_| rules::source_not_found(id))?;
        rules::clear_source_uses(tx.doc_mut(), id);
        Ok(())
    })
}

/// The prices and notices the latest value of `source` leads to, read
/// from the vault as it is.
fn spread(vault: &Session, source: &DataSource, ru: bool, delivered_at: &str) -> Spread {
    rules::spread_value(
        &vault.doc,
        source,
        |currency| crate::money::rates::per_usd(&vault.doc, currency),
        ru,
        delivered_at,
    )
}

/// Write a changed source and merge the prices it sets: one transact.
fn store_source(vault: &mut Session, source: DataSource, spread: &Spread) -> Result<()> {
    vault.transact(|tx| {
        let id = source.id.clone();
        tx.collection::<DataSource>()
            .update(&id, |slot| *slot = source)?;
        if !spread.points.is_empty() {
            merge_prices(&mut tx.doc_mut().price_cache, &spread.points);
        }
        Ok(())
    })
}

/// Record a fetch. An error keeps the value and counts a failure
/// ([`BROKEN_AFTER`](crate::model::datasource::BROKEN_AFTER) in a row mark the source
/// broken, with a notice); a number that jumped
/// [`JUMP_FACTOR`](crate::model::datasource::JUMP_FACTOR) times waits for
/// confirmation; any other value becomes the latest and goes to its uses.
pub fn apply_source_fetch(
    vault: &mut Session,
    id: &str,
    fetched: std::result::Result<crate::datasource::Extracted, String>,
    ru: bool,
) -> Result<Vec<NoticeRecord>> {
    let mut source = rules::data_source(&vault.doc, id)
        .cloned()
        .ok_or_else(|| rules::source_not_found(id))?;
    let at = now_iso();
    let mut outcome = rules::record_fetch(&mut source, fetched, &at, ru);
    let spread = if outcome.spread {
        spread(vault, &source, ru, &at)
    } else {
        Spread::default()
    };
    store_source(vault, source, &spread)?;
    outcome.notices.extend(spread.notices);
    Ok(outcome.notices)
}

/// Take or drop the value waiting after a jump.
pub fn confirm_source_value(
    vault: &mut Session,
    id: &str,
    accept: bool,
    ru: bool,
) -> Result<Vec<NoticeRecord>> {
    let mut source = rules::data_source(&vault.doc, id)
        .cloned()
        .ok_or_else(|| rules::source_not_found(id))?;
    let taken = rules::settle_pending(&mut source, accept)?;
    let spread = if taken {
        spread(vault, &source, ru, &now_iso())
    } else {
        Spread::default()
    };
    store_source(vault, source, &spread)?;
    Ok(spread.notices)
}
