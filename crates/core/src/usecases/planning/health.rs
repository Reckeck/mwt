//! The two writes of the capital checkup (plan C4): the freedom rate and the
//! monthly notice. The light and the report are in `reports::health`, the
//! figures and texts in `planning::health`.

use crate::Amount;
use crate::calendar::{self, Calendar};
use crate::error::Result;
use crate::model::notify::notice;
use crate::model::notify::notice_title;
use crate::model::notify::{NoticeCategory, NoticeRecord};
use crate::model::planning::{HealthColor, HealthSnapshot};
use crate::planning::health::{check_freedom_rate, color_text, reason_text};
use crate::reports::health::capital_health;
use crate::service::UnlockedVault;

/// Set the yearly return the freedom potential assumes; `None` = the key rate.
pub fn set_freedom_rate(vault: &mut UnlockedVault, rate: Option<Amount>) -> Result<()> {
    check_freedom_rate(rate)?;
    vault.transact(|tx| {
        tx.doc_mut().settings.freedom_rate = rate;
        Ok(())
    })
}

/// The monthly checkup notice (decision 28): once a month, only when the
/// colour or the reasons changed since the last check. The check itself
/// is remembered either way.
pub(crate) fn health_notice(
    vault: &mut UnlockedVault,
    now: i64,
    zone: &str,
    ru: bool,
) -> Result<Option<NoticeRecord>> {
    let month = calendar::month_key(Calendar::new(zone).date_of(now));
    if vault
        .doc
        .health_last
        .as_ref()
        .is_some_and(|last| last.month == month)
    {
        return Ok(None);
    }
    let report = capital_health(&vault.books(), now, zone)?;
    let snapshot = HealthSnapshot {
        month: month.clone(),
        color: report.color,
        reasons: report.reasons.iter().map(|item| item.code).collect(),
    };
    let changed = match &vault.doc.health_last {
        None => snapshot.color != HealthColor::NoData,
        Some(last) => last.color != snapshot.color || last.reasons != snapshot.reasons,
    };
    vault.transact(|tx| {
        tx.doc_mut().health_last = Some(snapshot);
        Ok(())
    })?;
    if !changed {
        return Ok(None);
    }
    let colour = color_text(report.color, ru);
    let body = match report.reasons.first() {
        Some(first) if first.level > HealthColor::Green || report.color == HealthColor::Green => {
            if ru {
                format!(
                    "Светофор капитала: {colour}. {}",
                    reason_text(first.code, ru)
                )
            } else {
                format!("Capital light: {colour}. {}", reason_text(first.code, ru))
            }
        }
        _ if ru => format!("Светофор капитала: {colour}."),
        _ => format!("Capital light: {colour}."),
    };
    Ok(Some(notice(
        NoticeCategory::Health,
        format!("health:{month}"),
        notice_title(NoticeCategory::Health, ru),
        body,
        Some(crate::routes::MONEY_HEALTH.into()),
        calendar::iso(now),
    )))
}
