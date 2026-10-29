import { useI18n } from 'vue-i18n'

const MINUTE_MS = 60_000
const HOUR_MS = 60 * MINUTE_MS
const DAY_MS = 24 * HOUR_MS

/** `2 min ago`: how long ago a sync was, in the units a person reads (minutes, hours, days). */
export function useSyncAgo() {
  const { t } = useI18n()
  return (iso: string, now: number): string => {
    const age = Math.max(0, now - new Date(iso).getTime())
    if (age < MINUTE_MS) return t('sync.ago.now')
    if (age < HOUR_MS) return t('sync.ago.minutes', { count: Math.floor(age / MINUTE_MS) })
    if (age < DAY_MS) return t('sync.ago.hours', { count: Math.floor(age / HOUR_MS) })
    return t('sync.ago.days', { count: Math.floor(age / DAY_MS) })
  }
}
