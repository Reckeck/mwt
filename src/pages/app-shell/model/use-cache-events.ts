import { useQueryClient } from '@tanstack/vue-query'

import { useBackendEvent } from '@/shared/api/use-backend-event'
import { notify } from '@/shared/lib/notify'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Every key a planning or ledger change can touch. */
export const PLANNING_AND_LEDGER = [...PLANNING_KEYS, ...LEDGER_KEYS]

/**
 * What the background (scheduler, poller, price refresh) changed, so the
 * screens drop the stale data: one table of event → query keys.
 */
export function useCacheEvents() {
  const queryClient = useQueryClient()
  const invalidate = (keys: ReadonlyArray<readonly unknown[]>) => {
    for (const queryKey of keys) void queryClient.invalidateQueries({ queryKey })
  }

  // The background poller never writes transactions, it only stages fills.
  useBackendEvent('bybit-import-pending', () => invalidate([['pending-imports']]))

  // A refresh that priced operations saved without a price changes their
  // badges and every cost basis built on them.
  useBackendEvent<RefreshResult>('prices-updated', (result) => {
    if (result.restamped)
      invalidate([['transactions'], ['store'], ['store-pnl'], ['store-history'], ['history'], ['summary']])
  })

  useBackendEvent('automation-created', () => invalidate(PLANNING_AND_LEDGER))
  // Runs in the background may leave questions, queued operations or sleeping runs.
  useBackendEvent('workflows-ran', () => invalidate([WORKFLOW_KEYS.runs, WORKFLOW_KEYS.queue, WORKFLOW_KEYS.waits]))
  useBackendEvent('price-alert', () => invalidate([['price-alerts']]))
  useBackendEvent('reminder-due', () => invalidate([['reminders']]))
  useBackendEvent('enrichment-ready', () => {
    invalidate([['enrichments']])
    notify.info('enrichment.event')
  })

  return { invalidate }
}
