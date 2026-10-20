export const NO_RUNS: AutomationRun[] = []

/**
 * Runs per automation, the rejected ones among them and the ones booked
 * approximately: built once per list, not filtered again for every read in
 * every card.
 */
export function groupRuns(runs: AutomationRun[]) {
  const all = new Map<string, AutomationRun[]>()
  const rejected = new Map<string, AutomationRun[]>()
  const unconfirmed = new Map<string, AutomationRun[]>()
  for (const run of runs) {
    push(all, run)
    if (typeof run.outcome === 'object' && 'rejected' in run.outcome) push(rejected, run)
    if (run.estimated) push(unconfirmed, run)
  }
  return { all, rejected, unconfirmed }
}

function push(byId: Map<string, AutomationRun[]>, run: AutomationRun) {
  const list = byId.get(run.automation_id)
  if (list) list.push(run)
  else byId.set(run.automation_id, [run])
}
