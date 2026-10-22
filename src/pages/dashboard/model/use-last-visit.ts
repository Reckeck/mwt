import { useLocalStorage } from '@vueuse/core'

import { dayKey } from '@/shared/lib/period'

/** A week back: what «Что изменилось» covers on the first visit. */
const FIRST_VISIT_DAYS = 7

/**
 * The day Home was last opened before today (Home 1, «Что изменилось с …»).
 * Visits within one day keep the same answer; the next day moves it on.
 * It is this device's memory, not the vault's.
 */
export function useLastVisit(now = new Date()) {
  const stored = useLocalStorage('trade-app:home-visit', { seen: '', since: '' })
  const today = dayKey(now)
  if (stored.value.seen !== today) {
    const fallback = dayKey(new Date(now.getTime() - FIRST_VISIT_DAYS * 86_400_000))
    stored.value = { seen: today, since: stored.value.seen || fallback }
  }
  return stored.value.since
}
