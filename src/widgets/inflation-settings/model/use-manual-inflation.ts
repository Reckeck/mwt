import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'

import { INFLATION_OVERVIEW_KEY, useInflationOverview } from '@/entities/currency'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * The group's flat rates (`Settings.inflation_manual` over the shipped table,
 * read through the overview): an editable copy of `code → percent`, written on
 * every change. They live in the vault and sync, so every device of the group
 * computes with the same table; they back up Auto before the first reading.
 */
export function useManualInflation() {
  const { t } = useI18n()
  const settings = useSettingsStore()
  const queryClient = useQueryClient()
  const { query } = useInflationOverview()

  const table = ref<Record<string, string>>({})
  watch(
    () => query.data.value,
    (rows) => {
      table.value = Object.fromEntries(
        (rows ?? []).filter((row) => row.manual != null).map((row) => [row.currency, String(row.manual)]),
      )
    },
    { immediate: true, deep: true },
  )

  /** `quiet`: the caller reports the change itself (a removal offers `Undo`). */
  async function persist(quiet = false) {
    const clean: Record<string, string> = {}
    for (const [code, rate] of Object.entries(table.value)) {
      const value = Number(rate.replace(',', '.'))
      if (code && Number.isFinite(value)) clean[code] = String(value)
    }
    try {
      await settings.updateVault({ inflation_manual: clean })
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
