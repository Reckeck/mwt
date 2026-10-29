import { type Ref, computed } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** `Import every hour`: what the switch turns on. */
export const HOURLY = 3600

/**
 * The import tab of an exchange (Wallets-2 · 8): the last sync and the
 * background interval, which the switch turns on (hourly) or off.
 */
export function useExchangeImport(store: Ref<Store>) {
  const queryClient = useQueryClient()
  const runs = useQuery({
    // no placeholder: another exchange's runs
    queryKey: computed(() => ['import-runs', store.value.id]),
    queryFn: () => call<ImportRun[]>('list_import_runs', { storeId: store.value.id }),
  })
  /** The last sync that went through; a failed one only explains the status line. */
  const lastRun = computed(() => runs.data.value?.[0] ?? null)
  const lastOk = computed(() => runs.data.value?.find((run) => !run.error) ?? null)
  const interval = computed(() => store.value.exchange?.import_interval_sec ?? 0)

  const setInterval = useMutation({
    mutationFn: (seconds: number) =>
      call('set_import_interval', { input: { store_id: store.value.id, interval_sec: seconds } }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['store'] }),
    meta: { error: 'stores.saveFailed', silent: 'the switch shows the new state' },
  })
  const toggle = (on: boolean) => setInterval.mutate(on ? HOURLY : 0)

  return { lastRun, lastOk, interval, toggle, saving: setInterval.isPending }
}
