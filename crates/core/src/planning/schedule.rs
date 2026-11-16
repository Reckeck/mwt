//! Pure dedupe of automation moments by local date.

use std::collections::HashSet;

use crate::calendar::Calendar;
use crate::time::LocalDate;

/// Local dates (in `cal`'s zone) already processed for one workflow, from
/// `(workflow id, RFC3339 moment)` pairs. Unparsable moments are ignored.
pub fn processed_dates(
    processed: &HashSet<(String, String)>,
    workflow_id: &str,
    cal: &Calendar,
) -> HashSet<LocalDate> {
    processed
        .iter()
        .filter(|(id, _)| id == workflow_id)
        .filter_map(|(_, moment)| cal.date_of_iso(moment))
        .collect()
}

/// Moments whose local date is not yet processed, order kept.
pub fn fresh_moments(moments: &[i64], done: &HashSet<LocalDate>, cal: &Calendar) -> Vec<i64> {
    moments
        .iter()
        .copied()
        .filter(|secs| !done.contains(&cal.date_of(*secs)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::iso;

    // 2026-03-15 02:00:00Z
    const BANGKOK_9AM: i64 = 1_773_540_000;
    const HOUR: i64 = 3600;
    const DAY: i64 = 86_400;

    fn processed_at(secs: i64) -> HashSet<(String, String)> {
        HashSet::from([("wf".to_string(), iso(secs))])
    }

    #[test]
    fn moments_on_a_processed_local_date_are_dropped() {
        let bangkok = Calendar::new("Asia/Bangkok");
        let moscow = Calendar::new("Europe/Moscow");
        let done = processed_dates(&processed_at(BANGKOK_9AM), "wf", &bangkok);
        assert_eq!(done.len(), 1);
        // 15 Mar 06:00Z = 09:00 MSK, same local date as 09:00 Bangkok.
        let moments = [BANGKOK_9AM + 4 * HOUR, BANGKOK_9AM + DAY + 4 * HOUR];
        assert_eq!(
            fresh_moments(&moments, &done, &moscow),
            vec![BANGKOK_9AM + DAY + 4 * HOUR]
        );
    }

    #[test]
    fn other_workflows_and_garbage_are_ignored() {
        let cal = Calendar::new("Europe/Berlin");
        let mut processed = processed_at(BANGKOK_9AM);
        processed.insert(("other".to_string(), iso(BANGKOK_9AM + DAY)));
        processed.insert(("wf".to_string(), "not a date".to_string()));
        assert_eq!(processed_dates(&processed, "wf", &cal).len(), 1);
    }

    #[test]
    fn nothing_processed_keeps_every_moment() {
        let cal = Calendar::new("UTC");
        let moments = [1, DAY, 2 * DAY];
        assert_eq!(fresh_moments(&moments, &HashSet::new(), &cal), moments);
    }
}
