import { computed, type Ref, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'

import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

import { baseOf, pairOf } from '../lib/pair'

interface TerminalDrawingsOptions {
  storeId: () => string
  queryKey: Ref<unknown[]>
  state: () => TerminalState | undefined
  /** The pair on the chart: clearing it from outside remounts the chart. */
  active: Ref<string>
  fail: (err: unknown) => void
}

/**
 * Chart drawings per pair: saved to the vault debounced (a drag fires many
 * changes), cleared from the watchlist menu with «Undo».
 */
export function useTerminalDrawings({ storeId, queryKey, state, active, fail }: TerminalDrawingsOptions) {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  /** Bumped to remount the chart when its drawings change from outside it. */
  const chartNonce = ref(0)
  const pending = new Map<string, string>()

  function remember(batch: Array<[string, string]>) {
    queryClient.setQueryData<TerminalState>(queryKey.value, (old) => {
      if (!old) return old
      const drawings = { ...old.drawings }
      for (const [symbol, json] of batch) {
        if (json.trim() === '[]' || !json.trim()) delete drawings[symbol]
        else drawings[symbol] = json
      }
      return { ...old, drawings }
    })
  }

  /** Write one pair's drawings and show them; `json` of `'[]'` clears them. */
  async function write(pair: string, json: string) {
    await call('save_terminal_drawings', { input: { store_id: storeId(), symbol: pair, json } })
    remember([[pair, json]])
    if (pair === active.value) chartNonce.value += 1
  }

  async function clear(pair: string) {
    pending.delete(pair)
    const before = state()?.drawings[pair]
    try {
      await write(pair, '[]')
      notify.success(t('terminal.drawingsCleared', { symbol: baseOf(pair) }), {
        action: before ? { label: t('actions.undo'), onClick: () => void write(pair, before).catch(fail) } : undefined,
      })
    } catch (err) {
      fail(err)
    }
  }

  const flush = useDebounceFn(async () => {
    const batch = [...pending.entries()]
    pending.clear()
    try {
      for (const [symbol, json] of batch) {
        await call('save_terminal_drawings', { input: { store_id: storeId(), symbol, json } })
      }
      remember(batch)
    } catch (err) {
      fail(err)
    }
  }, 1000)

  /** The chart reports drawings by its base ticker. */
  function onDrawings(symbol: string, json: string) {
    pending.set(pairOf(symbol), json)
    void flush()
  }

  /** The chart keys drawings by its base ticker; the vault by pair. */
  const saved = computed(() => {
    const out: Record<string, string> = {}
    for (const [pair, json] of Object.entries(state()?.drawings ?? {})) {
      if (json) out[baseOf(pair)] = json
    }
    return out
  })

  return { chartNonce, clear, onDrawings, saved }
}
