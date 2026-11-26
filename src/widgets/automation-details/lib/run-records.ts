/** The run records of an automation by run id: the card's rows are journal entries with the same ids. */
export function recordsById(records: readonly AutomationRun[] | undefined): Map<string, AutomationRun> {
  return new Map((records ?? []).map((record) => [record.id, record]))
}

/** A run still waits for the user: a refusal to retry or skip, or an approximate amount to confirm. */
export function runNeedsAction(record: AutomationRun | undefined): record is AutomationRun {
  if (!record || record.outcome === 'skipped') return false
  return 'rejected' in record.outcome || (record.estimated && 'created' in record.outcome)
}
