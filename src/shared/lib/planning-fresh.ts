/** A limit made a moment ago: its first «превышен» notice would only repeat what the screen already shows. */
const FRESH_MS = 60_000
const created = new Map<string, number>()

export function markFreshBudget(id: string): void {
  created.set(id, Date.now())
}

/** Whether a notice key (`budget:<id>:<period>:<level>`) belongs to a limit created just now. */
export function isFreshBudgetNotice(key: string): boolean {
  const id = key.split(':')[1]
  const at = key.startsWith('budget:') && id ? created.get(id) : undefined
  return at !== undefined && Date.now() - at < FRESH_MS
}
