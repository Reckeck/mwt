import { useQueryClient } from '@tanstack/vue-query'

import { PRICED_KEYS } from '@/features/price-refresh'
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
  // New quotes or rates, whoever fetched them (button, tray, the device's timer, a rate the
  // transaction form needed): every figure in a currency re-reads once, from here.
  useBackendEvent<RefreshResult>('prices-updated', (result) => {
    queryClient.setQueryData(['refresh'], result)
    invalidate(PRICED_KEYS)
    if (result.restamped)
      invalidate([['transactions'], ['store'], ['store-pnl'], ['store-history'], ['history'], ['summary']])
  })

  // Inflation or the key rate came in (the daily fetch, `Refresh`, a mode change).
  useBackendEvent('rates-updated', () => invalidate([['inflation-overview'], ['report'], ['interest-tax']]))
  // Own data sources (plan I2) feed inflation and asset prices.
  useBackendEvent('sources-updated', () => {
    invalidate([['data-sources'], ['inflation-overview'], ['report']])
    invalidate(PRICED_KEYS)
  })
  useBackendEvent('automation-created', () => invalidate(PLANNING_AND_LEDGER))
  // Runs in the background may leave questions, queued operations or sleeping runs.
  useBackendEvent('workflows-ran', () => invalidate([WORKFLOW_KEYS.runs, WORKFLOW_KEYS.queue, WORKFLOW_KEYS.waits]))
  useBackendEvent('price-alert', () => invalidate([['price-alerts']]))
  useBackendEvent('reminder-due', () => invalidate([['reminders']]))
  // The scheduler made or moved automatic reminders.
  useBackendEvent('reminders-changed', () => invalidate([['reminders']]))
  useBackendEvent('enrichment-ready', () => {
    invalidate([['enrichments']])
    notify.info('enrichment.event')
  })

  return { invalidate }
}
