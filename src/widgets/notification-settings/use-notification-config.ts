import { computed } from 'vue'

import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

import { DEFAULT_LEAD_DAYS } from './notification-settings.constants'

/** The vault's notification settings and the ways to change them, shared by the short list and the details. */
export function useNotificationConfig() {
  const settings = useSettingsStore()

  const current = computed<NotificationSettings>(
    () =>
      settings.vault?.notifications ?? {
        system: true,
        hide_details_when_locked: false,
        default_time: '10:00',
        categories: [],
      },
  )

  function category(name: NoticeCategory): NoticeCategoryCfg {
    return (
      current.value.categories.find((entry) => entry.category === name) ?? {
        category: name,
        enabled: true,
        lead_days: DEFAULT_LEAD_DAYS[name] ?? 0,
        auto_reminders: true,
      }
    )
  }

  async function save(patch: Partial<NotificationSettings>) {
    try {
      await settings.updateVault({ notifications: { ...current.value, ...patch } })
      notify.saved()
    } catch (err) {
      notify.error(err, 'settings.saveFailed')
    }
  }

  function patchCategory(name: NoticeCategory, patch: Partial<NoticeCategoryCfg>) {
    const next = { ...category(name), ...patch }
    const others = current.value.categories.filter((entry) => entry.category !== name)
    void save({ categories: [...others, next] })
  }

  return { current, category, save, patchCategory }
}
