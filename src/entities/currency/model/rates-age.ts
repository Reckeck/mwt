import { computed } from 'vue'
import { useOnline } from '@vueuse/core'

import { localDate } from '@/shared/lib/planning'

import { useConversions } from './conversions'

/**
 * How old the rates behind the sums on screen are (Currency-2 · 8): the oldest
 * rate among the currencies the vault holds, coins aside (they follow the
 * prices). `stale` when it is from an earlier day — the figures then carry
 * «курс 20 окт.»; `offline` from the device. Both go once the rates come in.
 */
export function useRatesAge() {
  const { table } = useConversions(true)
  const online = useOnline()

  const at = computed(() => {
    const stamps = (table.data.value?.rows ?? [])
      .filter((row) => row.tracked && !row.crypto && row.updated_at)
      .map((row) => row.updated_at)
      .sort()
    return stamps[0] ?? null
  })
  const stale = computed(() => !!at.value && localDate(new Date(at.value)) < localDate(new Date()))
  /** Whole days since the rates, by the calendar: yesterday's evening is 1. */
  const days = computed(() => {
    if (!at.value) return 0
    const day = (date: Date) => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate())
    return Math.round((day(new Date()) - day(new Date(at.value))) / 86_400_000)
  })

  return { at, stale, days, offline: computed(() => !online.value) }
}
