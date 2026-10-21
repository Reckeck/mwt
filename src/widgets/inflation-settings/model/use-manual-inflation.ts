import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { INFLATION_OVERVIEW_KEY } from '@/entities/currency'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * The user's flat rates (`AppSettings.inflation`): an editable copy of
 * `code → percent`, written on every change. They stay outside the vault, so
 * they work locked too, and back up Auto before the first reading.
 */
export function useManualInflation() {
  const { t } = useI18n()
  const settings = useSettingsStore()
  const queryClient = useQueryClient()

  const table = ref<Record<string, string>>({})
  watch(
    () => settings.app?.inflation,
    (saved) => {
      table.value = Object.fromEntries(Object.entries(saved ?? {}).map(([code, rate]) => [code, String(rate)]))
    },
    { immediate: true, deep: true },
  )

  /** `quiet`: the caller reports the change itself (a removal offers «Отменить»). */
  async function persist(quiet = false) {
    const clean: Record<string, string> = {}
    for (const [code, rate] of Object.entries(table.value)) {
      const value = Number(rate.replace(',', '.'))
      if (code && Number.isFinite(value)) clean[code] = String(value)
    }
    try {
      await settings.updateApp({ inflation: clean })
      if (!quiet) notify.saved()
      for (const queryKey of [['report'], INFLATION_OVERVIEW_KEY]) void queryClient.invalidateQueries({ queryKey })
    } catch (err) {
      notify.error(err, 'settings.saveFailed')
    }
  }

  function add(code: string) {
    table.value = { ...table.value, [code]: table.value[code] ?? '0' }
    void persist()
  }

  function remove(code: string) {
    const removed = table.value[code]
    if (removed === undefined) return
    const { [code]: _gone, ...rest } = table.value
    table.value = rest
    void persist(true)
    notify.success('inflation.rowRemoved', {
      description: code.toUpperCase(),
      action: {
        label: t('actions.undo'),
        onClick: () => {
          table.value = { ...table.value, [code]: removed }
          void persist(true)
        },
      },
    })
  }

  return { table, persist, add, remove }
}
