<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Combobox, type ComboboxOption } from '@/shared/ui/combobox'
import { Dialog } from '@/shared/ui/dialog'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call } from '@/shared/api/ipc'
import { useCategories, useStoreTree } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree, withoutLoyalty } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import { WORKDAY_SHIFTS } from '../../automation-edit.constants'
import { SALARY_SHARE_PCT, salaryInput } from '../../lib/salary-plan'

/**
 * «Зарплата» (Auto-2 · 9): the ready-made start already filled in — the
 * income category, the wallet, the usual amount, the day and where a day off
 * moves it. The switch adds «10 % на Наличные» as the scheme's second step.
 */
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [automation: Automation] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()
const categories = useCategories()
const tree = useStoreTree()

const DAYS = Array.from({ length: 31 }, (_, index) => String(index + 1))

const category = ref('')
const storeId = ref('')
const varies = ref(true)
const amount = ref('')
const currency = ref('')
const day = ref('25')
const shift = ref<WorkdayShift>('back')
const share = ref(true)

const nodes = computed(() => withoutLoyalty(tree.data.value ?? []))
const flat = computed(() => flattenStoreTree(nodes.value))
/** Where «10 %» goes: the first cash wallet; without one there is no switch. */
const cash = computed(() => flat.value.find((node) => node.store.kind === 'cash' && !node.store.archived)?.store ?? null)
const incomes = computed(() => (categories.data.value ?? []).filter((entry) => entry.group === 'income' && !entry.archived))
const options = computed<ComboboxOption[]>(() => incomes.value.map((entry) => ({ value: entry.key, label: categoryLabel(entry, t) })))

watch(
  open,
  (isOpen) => {
    if (!isOpen) return
    category.value = incomes.value.find((entry) => entry.key === 'salary')?.key ?? incomes.value[0]?.key ?? ''
    storeId.value = flat.value.find((node) => node.store.kind === 'bank' && !node.store.archived)?.store.id ?? flat.value[0]?.store.id ?? ''
    varies.value = true
    amount.value = ''
    currency.value = settings.baseCurrency
    day.value = '25'
    shift.value = 'back'
    share.value = true
  },
  { immediate: true },
)

const form = useFormValidation(
  () => ({
    category: required(category.value, 'pick'),
    store: required(storeId.value, 'pick'),
    amount: positiveAmount(amount.value),
  }),
  { resetOn: open },
)

const setMode = (value: unknown) => {
  if (value === 'fixed' || value === 'varies') varies.value = value === 'varies'
}

const save = useMutation({
  mutationFn: async () => {
    const name = options.value.find((entry) => entry.value === category.value)?.label ?? t('salary.title')
    const draft = {
      category: category.value,
      storeId: storeId.value,
      varies: varies.value,
      amount: amount.value,
      currency: currency.value,
      day: Number(day.value),
      shift: shift.value,
    }
    const automation = await call<Automation>('create_automation', { input: salaryInput(draft, name) })
    if (share.value && cash.value) {
      const params = { category: category.value, pct: String(SALARY_SHARE_PCT), target: cash.value.id }
      await call('workflow_from_template', { input: { template_id: 'pay_yourself_first', params } })
    }
    return automation
  },
  onSuccess: (automation) => {
    for (const queryKey of [...PLANNING_KEYS, ['workflows']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', automation)
  },
  meta: {
    error: 'automation.saveFailed',
    success: (data) => ({ text: t('automation.created'), description: (data as Automation).name }),
  },
})
</script>

<template>
  <Dialog v-model:open="open" size="form" :title="t('salary.title')" :description="t('salary.subtitle')">
    <form id="salary-form" class="grid gap-4" data-tour="salary-form" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <FormField for="salary-source" :label="t('salary.source')" required :hint="t('salary.sourceHint')" :error="form.error('category')">
          <Combobox id="salary-source" v-model="category" :options="options" />
        </FormField>
        <FormField for="salary-store" :label="t('salary.store')" required :error="form.error('store')">
          <StoreNodeSelect id="salary-store" v-model="storeId" :nodes="nodes" class="w-full" />
        </FormField>
      </div>
      <div class="grid gap-2">
        <span class="text-sm font-medium">{{ t('salary.amount') }}</span>
        <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-full sm:w-fit" :model-value="varies ? 'varies' : 'fixed'" @update:model-value="setMode">
          <ToggleGroupItem value="fixed" class="px-3" data-salary-mode="fixed">{{ t('salary.fixed') }}</ToggleGroupItem>
          <ToggleGroupItem value="varies" class="px-3" data-salary-mode="varies">{{ t('salary.varies') }}</ToggleGroupItem>
        </ToggleGroup>
      </div>
      <FormField
        for="salary-amount"
        :label="varies ? t('salary.usual') : t('salary.sum')"
        :required="!varies"
        :hint="varies ? t('salary.usualHint') : undefined"
        :error="form.error('amount')"
      >
        <AmountInput id="salary-amount" v-model="amount">
          <template #suffix>
            <CurrencyPicker id="salary-currency" v-model="currency" label="" chip />
          </template>
        </AmountInput>
      </FormField>
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <FormField for="salary-day" :label="t('salary.day')" required>
          <Select id="salary-day" v-model="day">
            <SelectItem v-for="entry in DAYS" :key="entry" :value="entry">{{ t('automationWizard.dayOption', { day: entry }) }}</SelectItem>
          </Select>
        </FormField>
        <FormField for="salary-shift" :label="t('salary.dayOff')">
          <Select id="salary-shift" v-model="shift">
            <SelectItem v-for="entry in WORKDAY_SHIFTS" :key="entry" :value="entry">{{ t(`salary.shift.${entry}`) }}</SelectItem>
          </Select>
        </FormField>
      </div>
      <SwitchRow
        v-if="cash"
        id="salary-share"
        v-model="share"
        plain
        :label="t('salary.share', { pct: SALARY_SHARE_PCT, name: cash.name })"
        :hint="t('salary.shareHint')"
      />
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="salary-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
