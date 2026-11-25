import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useSyncStatus } from './use-sync-status'

/** The sync state in one line for the profile menu: on or off, busy, an unreliable clock, and how many devices are in sync. */
export function useSyncIndicator() {
  const { t } = useI18n()
  const { status, sessions, connected } = useSyncStatus()

  const enabled = computed(() => Boolean(status.data.value?.enabled))
  const busy = computed(() =>
    sessions.value.some((session) => session.phase === 'exchanging' || session.phase === 'connecting'),
  )
  const unreliable = computed(() => sessions.value.some((session) => session.unreliable))
  const hasConnected = computed(() => connected.value.length > 0)
  const label = computed(() => {
    const role = status.data.value?.role ?? 'off'
    if (connected.value.length) return t('sync.indicator.connected', { count: connected.value.length })
    return t(`sync.role.${role}`)
  })
  return { enabled, busy, unreliable, connected: hasConnected, label }
}
