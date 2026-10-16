<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { CategorySelect } from '@/features/category-select'
import { StoreNodeSelect } from '@/entities/store'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { firstError, integerInRange, nonNegativeAmount, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { Switch } from '@/shared/ui/switch'

import MeterFields from './MeterFields.vue'
import { type MeterMode, useFormulaPreview, zoneKey, zoneList } from './meter-formula'
import { UTILITY_KINDS, type UtilityKindName } from './utility-edit.constants'
import type { UtilityDialogProps } from './utility-edit.types'

const props = withDefaults(defineProps<UtilityDialogProps>(), { item: null, propertyId: '' })
const open = defineModel<boolean>('open', { required: true })

const { t, locale } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))
/** Bills belong to a property; only top-level property wallets qualify. */
const properties = computed(() => nodes.value.filter((node) => node.store.kind === 'property' && !node.store.parent_id))
const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })

const name = ref('')
const kind = ref<UtilityKindName>('metered')
const unit = ref('kWh')
const tariff = ref('')
const meterMode = ref<MeterMode>('tariff')
const formula = ref('')
const zones = ref('')
const preview = useFormulaPreview({ tariff, mode: meterMode, formula, zones })
const amount = ref('')
const deposit = ref('')
const dueDay = ref('1')
const automationId = ref('')
const currency = ref('')
const storeId = ref('')
const propertyId = ref('')
const category = ref('utilities')
const active = ref(true)

watch(open, (isOpen) => {
  if (!isOpen) return
  const item = props.item
  name.value = item?.name ?? ''
  const current = item?.kind ?? { metered: { unit: 'kWh', tariff: '', formula: null, zones: [] } }
  kind.value = 'metered' in current ? 'metered' : 'static' in current ? 'static' : 'housing'
  unit.value = 'metered' in current ? current.metered.unit : 'kWh'
  tariff.value = 'metered' in current ? current.metered.tariff : ''
  formula.value = 'metered' in current ? (current.metered.formula ?? '') : ''
  meterMode.value = formula.value ? 'formula' : 'tariff'
  zones.value = 'metered' in current ? current.metered.zones.join(', ') : ''
  amount.value = 'static' in current ? current.static.amount : 'housing' in current ? current.housing.amount : ''
  deposit.value = 'housing' in current ? (current.housing.deposit ?? '') : ''
  dueDay.value = 'housing' in current ? String(current.housing.due_day) : '1'
  automationId.value = 'housing' in current ? (current.housing.automation_id ?? '') : ''
  currency.value = item?.currency ?? (settings.vault?.budget_currency || settings.baseCurrency)
  storeId.value = item?.store_id ?? ''
  propertyId.value = item?.property_id || props.propertyId || properties.value[0]?.store.id || ''
  category.value = item?.category ?? (kind.value === 'housing' ? 'rent_mortgage' : 'utilities')
  active.value = item?.active ?? true
})
watch(kind, (next) => {
  if (!props.item && next === 'housing' && category.value === 'utilities') category.value = 'rent_mortgage'
})

const utilityKind = computed<UtilityKind | null>(() => {
  if (kind.value === 'metered') {
    const own = meterMode.value === 'formula' ? formula.value.trim() : ''
    return {
      metered: { unit: unit.value.trim(), tariff: tariff.value.trim() || '0', formula: own || null, zones: zoneList(zones.value) },
    }
  }
  if (kind.value === 'static') return Number(amount.value) > 0 ? { static: { amount: amount.value.trim() } } : null
  const day = Number(dueDay.value)
  return Number(amount.value) > 0 && day >= 1 && day <= 31
    ? {
        housing: {
          amount: amount.value.trim(),
          deposit: deposit.value.trim() || null,
          due_day: day,
          automation_id: automationId.value || null,
        },
      }
    : null
})
/** Up to 4 zones, each named once — what `check_utility` refuses. */
const zonesError = computed(() => {
  const keys = zoneList(zones.value).map(zoneKey)
  return new Set(keys).size !== keys.length || keys.length > 4 ? t('utilities.zonesInvalid') : null
})
const formulaError = computed(() => {
  const issue = preview.value?.issue
  if (!issue || !formula.value.trim()) return null
  return t('utilities.formulaIssue', { position: issue.position, text: locale.value === 'ru' ? issue.text_ru : issue.text_en })
})
const form = useFormValidation(
  () => ({
    name: required(name.value),
    unit: kind.value === 'metered' ? required(unit.value) : null,
    tariff: kind.value === 'metered' && meterMode.value === 'tariff' ? positiveAmount(tariff.value) : null,
    zones: kind.value === 'metered' ? zonesError.value : null,
    formula: kind.value === 'metered' && meterMode.value === 'formula' ? firstError(required(formula.value), formulaError.value) : null,
    amount: kind.value === 'metered' ? null : positiveAmount(amount.value),
    deposit: kind.value === 'housing' ? nonNegativeAmount(deposit.value) : null,
    dueDay: kind.value === 'housing' ? firstError(required(dueDay.value), integerInRange(dueDay.value, 1, 31)) : null,
    currency: required(currency.value, 'pick'),
    property: required(propertyId.value, 'pick'),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    const input: UtilityInput = {
      name: name.value.trim(),
      active: active.value,
      kind: utilityKind.value as UtilityKind,
      currency: currency.value,
      property_id: propertyId.value,
      store_id: storeId.value || null,
      category: category.value,
    }
    return props.item
      ? call<UtilityItem>('update_utility_item', { input: { id: props.item.id, item: input } })
      : call<UtilityItem>('create_utility_item', { input })
  },
  onSuccess: () => {
    for (const queryKey of [['utilities'], ['utility-trend'], ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'utilities.saveFailed', success: () => props.item ? t('utilities.saved') : t('utilities.created') },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.item ? t('utilities.edit') : t('utilities.new')">
    <form id="utility-form" class="@container grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="grid gap-3 @sm:grid-cols-[1fr_11rem]">
        <FormField for="utility-name" :label="t('utilities.name')" required :error="form.error('name')">
          <Input id="utility-name" v-model="name" :placeholder="t('utilities.namePlaceholder')" />
        </FormField>
        <div class="grid gap-1.5">
          <Label for="utility-kind">{{ t('utilities.kind') }}</Label>
          <Select id="utility-kind" v-model="kind">
            <SelectItem v-for="entry in UTILITY_KINDS" :key="entry" :value="entry">{{ t(`utilities.kindName.${entry}`) }}</SelectItem>
          </Select>
        </div>
      </div>
      <p class="text-xs text-muted-foreground">{{ t(`utilities.kindHint.${kind}`) }}</p>

      <MeterFields
        v-if="kind === 'metered'"
        v-model:unit="unit"
        v-model:tariff="tariff"
        v-model:mode="meterMode"
        v-model:formula="formula"
        v-model:zones="zones"
        :currency="currency"
        :preview="preview"
        :unit-error="form.error('unit')"
        :tariff-error="form.error('tariff')"
        :zones-error="form.error('zones')"
        :formula-error="form.error('formula')"
      />
      <div v-else class="grid gap-3 @sm:grid-cols-3">
        <FormField for="utility-amount" :label="kind === 'housing' ? t('utilities.rent') : t('utilities.amount')" required :error="form.error('amount')">
          <AmountInput id="utility-amount" v-model="amount" />
        </FormField>
        <template v-if="kind === 'housing'">
          <FormField for="utility-deposit" :label="t('utilities.deposit')" :error="form.error('deposit')">
            <AmountInput id="utility-deposit" v-model="deposit" :placeholder="t('goal.optional')" />
          </FormField>
          <FormField for="utility-due" :label="t('utilities.dueDay')" required :error="form.error('dueDay')">
            <Input id="utility-due" v-model="dueDay" type="number" min="1" max="31" />
          </FormField>
        </template>
      </div>
      <div v-if="kind === 'housing'" class="grid gap-1.5">
        <Label for="utility-automation">{{ t('utilities.automation') }}</Label>
        <Select id="utility-automation" v-model="automationId">
          <SelectItem value="">{{ t('utilities.noAutomation') }}</SelectItem>
          <SelectItem v-for="entry in automations.data.value ?? []" :key="entry.id" :value="entry.id">{{ entry.name }}</SelectItem>
        </Select>
        <p class="text-xs text-muted-foreground">{{ t('utilities.automationHint') }}</p>
      </div>

      <div class="grid gap-3 @sm:grid-cols-3">
        <FormField for="utility-currency" :label="t('utilities.currency')" required :error="form.error('currency')">
          <CurrencyPicker id="utility-currency" v-model="currency" />
        </FormField>
        <FormField v-if="!props.propertyId" for="utility-property" :label="t('utilities.property')" required :error="form.error('property')">
          <StoreNodeSelect
            id="utility-property"
            v-model="propertyId"
            :nodes="properties"
            flat
            :placeholder="t('utilities.pickProperty')"
            :invalid="Boolean(form.error('property'))"
          />
          <p v-if="!properties.length" class="text-xs text-muted-foreground">{{ t('utilities.noProperty') }}</p>
        </FormField>
        <div class="grid gap-1.5">
          <Label for="utility-store">{{ t('utilities.store') }}</Label>
          <StoreNodeSelect id="utility-store" v-model="storeId" :nodes="nodes" :empty-label="t('utilities.anyStore')" />
        </div>
        <div class="grid gap-1.5">
          <Label for="utility-category">{{ t('utilities.category') }}</Label>
          <CategorySelect id="utility-category" v-model="category" :groups="['fixed']" />
        </div>
      </div>
      <div class="flex items-center justify-between gap-3 rounded-md border border-border px-3 py-2">
        <Label for="utility-active">{{ t('utilities.active') }}</Label>
        <Switch id="utility-active" v-model="active" />
      </div>
    </form>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="utility-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
