import { type InjectionKey, type Ref, computed, inject, provide, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import {
  firstError,
  integerInRange,
  nonNegativeAmount,
  positiveAmount,
  required,
  useFormValidation,
} from '@/shared/lib/form'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { useSettingsStore } from '@/shared/store/settings'

import { type MeterMode, useFormulaPreview, zoneKey } from '../meter-formula'
import type { UtilityKindName, ZoneCount } from '../utility-edit.constants'
import type { UtilityDialogProps } from '../utility-edit.types'
import { lastReadings } from './last-readings'

/**
 * Everything the item dialog holds (Wallets-3 · 14–17): the name and kind of
 * step 1, the fields of the kind on step 2, and the checks of the step on
 * screen. Editing skips step 1: the kind stays, the name sits on top of step 2.
 */
function setup(props: UtilityDialogProps, open: Ref<boolean>) {
  const { t, locale } = useI18n()
  const settings = useSettingsStore()
  const tree = useStoreTree()
  const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
  /** Bills belong to a property; only top-level property wallets qualify. */
  const properties = computed(() =>
    nodes.value.filter((node) => node.store.kind === 'property' && !node.store.parent_id),
  )

  const editing = computed(() => Boolean(props.item))
  const step = ref<1 | 2>(1)
  const name = ref('')
  const kind = ref<UtilityKindName>('metered')
  const unit = ref('')
  const tariff = ref('')
  const meterMode = ref<MeterMode>('tariff')
  const formula = ref('')
  /** Zone names as stored: kept when an edited meter has its own. */
  const zoneNames = ref<string[]>([])
  const amount = ref('')
  const deposit = ref('')
  const dueDay = ref('')
  const automationId = ref('')
  const currency = ref('')
  const storeId = ref('')
  const propertyId = ref('')
  const category = ref('utilities')

  const zoneCount = computed<ZoneCount | null>(() =>
    zoneNames.value.length === 0
      ? 'one'
      : zoneNames.value.length === 2
        ? 'two'
        : zoneNames.value.length === 3
          ? 'three'
          : null,
  )
  function setZoneCount(count: ZoneCount) {
    if (count === zoneCount.value) return
    zoneNames.value = count === 'one' ? [] : t(`utilityEdit.zoneNames.${count}`).split(', ')
  }

  watch(open, (isOpen) => {
    if (!isOpen) return
    const item = props.item
    step.value = item ? 2 : 1
    name.value = item?.name ?? ''
    const current = item?.kind ?? null
    kind.value = !current || 'metered' in current ? 'metered' : 'static' in current ? 'static' : 'housing'
    const meter = current && 'metered' in current ? current.metered : null
    unit.value = meter?.unit ?? (item ? '' : t('utilityEdit.defaultUnit'))
    tariff.value = meter?.tariff ?? ''
    formula.value = meter?.formula ?? ''
    meterMode.value = formula.value ? 'formula' : 'tariff'
    zoneNames.value = meter?.zones ?? []
    amount.value =
      current && 'static' in current
        ? current.static.amount
        : current && 'housing' in current
          ? current.housing.amount
          : ''
    const rent = current && 'housing' in current ? current.housing : null
    deposit.value = rent?.deposit ?? ''
    dueDay.value = rent ? String(rent.due_day) : ''
    automationId.value = rent?.automation_id ?? ''
    currency.value = item?.currency ?? propertyCurrency.value
    storeId.value = item?.store_id ?? ''
    propertyId.value = item?.property_id || props.propertyId || properties.value[0]?.store.id || ''
    category.value = item?.category ?? 'utilities'
  })
  /** A new item is in the money of the property's other items (Wallets-3 · 17), else the budget's. */
  const items = useQuery({
    queryKey: ['utilities'],
    queryFn: () => call<UtilityItem[]>('list_utility_items'),
    enabled: open,
  })
  const propertyCurrency = computed(
    () =>
      items.data.value?.find((entry) => entry.property_id === propertyId.value)?.currency ??
      (settings.vault?.budget_currency || settings.baseCurrency),
  )
  watch([propertyCurrency, open], () => {
    if (open.value && !editing.value) currency.value = propertyCurrency.value
  })
  watch(kind, (next) => {
    if (editing.value) return
    category.value = next === 'housing' ? 'rent_mortgage' : 'utilities'
    if (next === 'metered' && !unit.value) unit.value = t('utilityEdit.defaultUnit')
  })

  /** An edited meter checks its formula on its last real readings. */
  const trend = useQuery({
    // no placeholder: another item's readings must not check this one
    queryKey: computed(() => ['utility-trend', props.item?.id]),
    queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: props.item?.id }),
    enabled: computed(() => open.value && Boolean(props.item) && kind.value === 'metered'),
  })
  const sample = computed(() => (props.item ? lastReadings(trend.data.value, zoneNames.value) : null))
  const preview = useFormulaPreview({ tariff, mode: meterMode, formula, zones: zoneNames, sample })

  const utilityKind = computed<UtilityKind>(() => {
    if (kind.value === 'metered') {
      const own = meterMode.value === 'formula' ? formula.value.trim() : ''
      return {
        metered: {
          unit: unit.value.trim(),
          tariff: tariff.value.trim() || '0',
          formula: own || null,
          zones: zoneNames.value,
        },
      }
    }
    if (kind.value === 'static') return { static: { amount: amount.value.trim() } }
    return {
      housing: {
        amount: amount.value.trim(),
        deposit: deposit.value.trim() || null,
        due_day: Number(dueDay.value),
        automation_id: automationId.value || null,
      },
    }
  })

  /** Up to 4 zones, each named once — what `check_utility` refuses. */
  const zonesError = computed(() => {
    const keys = zoneNames.value.map(zoneKey)
    return new Set(keys).size !== keys.length || keys.length > 4 ? t('utilities.zonesInvalid') : null
  })
  const formulaError = computed(() => {
    const issue = preview.value?.issue
    if (!issue || !formula.value.trim()) return null
    return t('utilities.formulaIssue', {
      position: issue.position,
      text: locale.value === 'ru' ? issue.text_ru : issue.text_en,
    })
  })
  const form = useFormValidation(
    () => {
      const first = { name: required(name.value), property: required(propertyId.value, 'pick') }
      if (step.value === 1) return first
      const metered = kind.value === 'metered'
      const rent = kind.value === 'housing'
      return {
        ...(editing.value ? first : {}),
        unit: metered ? required(unit.value) : null,
        tariff: metered && meterMode.value === 'tariff' ? positiveAmount(tariff.value) : null,
        zones: metered ? zonesError.value : null,
        formula:
          metered && meterMode.value === 'formula' ? firstError(required(formula.value), formulaError.value) : null,
        amount: metered ? null : positiveAmount(amount.value),
        deposit: rent ? nonNegativeAmount(deposit.value) : null,
        dueDay: rent ? firstError(required(dueDay.value), integerInRange(dueDay.value, 1, 31)) : null,
      }
    },
    { resetOn: [open, step] },
  )

  const propertyName = computed(
    () => properties.value.find((node) => node.store.id === propertyId.value)?.store.name ?? '',
  )

  return {
    editing,
    step,
    name,
    kind,
    unit,
    tariff,
    meterMode,
    formula,
    zoneNames,
    zoneCount,
    setZoneCount,
    amount,
    deposit,
    dueDay,
    automationId,
    currency,
    storeId,
    propertyId,
    category,
    sample,
    preview,
    utilityKind,
    form,
    nodes,
    properties,
    propertyName,
  }
}

export type UtilityForm = ReturnType<typeof setup>
const KEY: InjectionKey<UtilityForm> = Symbol('utility-form')

export function provideUtilityForm(props: UtilityDialogProps, open: Ref<boolean>) {
  const state = setup(props, open)
  provide(KEY, state)
  return state
}

export function useUtilityForm() {
  const state = inject(KEY)
  if (!state) throw new Error('useUtilityForm outside UtilityDialog')
  return state
}
