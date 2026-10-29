import { useQueryClient } from '@tanstack/vue-query'

import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

import { sharedEventKeys } from '../api/shared-event-keys'

/** Everything an event's money can show up in: its own queries, the ledger, contact debts, terms. */
const STALE_KEYS: ReadonlyArray<readonly string[]> = [
  sharedEventKeys.all,
  ...LEDGER_KEYS,
  ...PLANNING_KEYS,
  ['contacts', 'balances'],
  ['debt-terms'],
  ['entity-card'],
]

/** Returns the function that refreshes everything a settlement or a purchase changed. */
export function useSharedEventSync() {
  const queryClient = useQueryClient()
  return () => {
    for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
  }
}
