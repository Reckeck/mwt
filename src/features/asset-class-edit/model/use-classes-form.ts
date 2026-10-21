import { computed, ref, watch, type Ref } from 'vue'

import { PRESET_CLASS_IDS } from '@/entities/asset-class'
import { i18n } from '@/shared/config/i18n'
import { parseDecimal } from '@/shared/lib/decimal'
import { firstError, required, useFormValidation } from '@/shared/lib/form'

import { useClassesSet } from './use-asset-class-edit'

export interface ClassRow {
  /** The form's own key: a new row has no id until the backend gives it one. */
  key: number
  id: string
  name: string
  target: string
  cap: boolean
}

const FULL = 100
const PRESET: Omit<ClassRow, 'key'>[] = [
  { id: 'local', name: '', target: '40', cap: false },
  { id: 'global', name: '', target: '40', cap: false },
  { id: 'risky', name: '', target: '20', cap: true },
]

const isPreset = (id: string) => (PRESET_CLASS_IDS as readonly string[]).includes(id)

/** The basket editor's rows, refilled from the saved list each time the dialog opens. */
export function useClassesForm(view: () => AssetClassesView | undefined, open: Ref<boolean>) {
  let nextKey = 0
  const rows = ref<ClassRow[]>([])
  const toRows = (list: Omit<ClassRow, 'key'>[]) => list.map((row) => ({ ...row, key: nextKey++ }))

  watch(open, (isOpen) => {
    if (!isOpen) return
    const classes = view()?.classes ?? []
    rows.value = toRows(classes.map((cls) => ({ id: cls.id, name: cls.name, target: cls.target_pct, cap: cls.cap })))
  })

  const total = computed(() => rows.value.reduce((sum, row) => sum + Number(parseDecimal(row.target) ?? 0), 0))

  const share = (value: string): string | null => {
    const parsed = parseDecimal(value)
    if (parsed === null) return i18n.global.t('validation.number')
    const number = Number(parsed)
    return number < 0 || number > FULL ? i18n.global.t('validation.range', { min: 0, max: FULL }) : null
  }
  const form = useFormValidation<string>(
    () => {
      const rules: Record<string, string | null> = {}
      for (const row of rows.value) {
        rules[`name-${row.key}`] = isPreset(row.id) ? null : required(row.name)
        rules[`pct-${row.key}`] = firstError(required(row.target), share(row.target))
      }
      // A rounding of 99.999… is not a typo the backend would accept either.
      rules.total =
        Math.abs(total.value - FULL) < 1e-9 ? null : i18n.global.t('assetClass.totalError', { total: total.value })
      return rules
    },
    { resetOn: open },
  )

  const set = useClassesSet()

  function add() {
    rows.value.push(...toRows([{ id: '', name: '', target: '0', cap: false }]))
  }
  function remove(key: number) {
    rows.value = rows.value.filter((row) => row.key !== key)
  }
  function resetPreset() {
    rows.value = toRows(PRESET)
  }

  function save() {
    const classes: AssetClass[] = rows.value.map((row) => ({
      id: row.id,
      name: row.name.trim(),
      target_pct: parseDecimal(row.target) ?? '0',
      cap: row.cap,
    }))
    const current = view()
    const previous = current && !current.preset ? current.classes : []
    set.mutate({ classes, previous }, { onSuccess: () => (open.value = false) })
  }

  return { rows, total, form, set, add, remove, resetPreset, save }
}
