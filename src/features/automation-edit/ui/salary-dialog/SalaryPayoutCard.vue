<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { WORKDAY_SHIFTS } from '../../automation-edit.constants'
import type { SalaryForMonth, SalaryPayout } from '../../lib/salary-plan'

/**
 * One payout of «Зарплата»: its name, the day, the month it is for, where a
 * day off moves it and the amount. The dialog shows it once or twice.
 */
const props = defineProps<{
  title: string
  /** Prefix of the field ids, so two cards do not share them. */
  idPrefix: string
  nameError?: string
  amountError?: string
}>()
const payout = defineModel<SalaryPayout>({ required: true })

const { t } = useI18n()

const DAYS = Array.from({ length: 31 }, (_, index) => String(index + 1))
const MONTHS: SalaryForMonth[] = ['previous', 'current']

/** A writable view of one field: the card edits a copy, the dialog holds the payouts. */
function field<Key extends keyof SalaryPayout>(key: Key) {
  return computed({
    get: () => payout.value[key],
    set: (value: SalaryPayout[Key]) => {
      payout.value = { ...payout.value, [key]: value }
    },
  })
}
const name = field('name')
const amount = field('amount')
const currency = field('currency')
const forMonth = field('forMonth')
const shift = field('shift')
const day = computed({
  get: () => String(payout.value.day),
  set: (value: string) => {
    payout.value = { ...payout.value, day: Number(value) }
  },
})

const setMode = (value: unknown) => {
  if (value === 'fixed' || value === 'varies') payout.value = { ...payout.value, varies: value === 'varies' }
}
const id = (part: string) => `${props.idPrefix}-${part}`
</script>

<template>
  <section class="grid grid-cols-2 items-start gap-x-4 gap-y-3 rounded-lg border p-4" :data-salary-payout="props.idPrefix">
    <h3 class="col-span-2 text-sm font-semibold">{{ props.title }}</h3>
    <FormField class="col-span-2 sm:col-span-1" :for="id('name')" :label="t('salary.name')" required :error="props.nameError">
      <Input :id="id('name')" v-model="name" />
    </FormField>
    <FormField :for="id('day')" :label="t('salary.day')" required>
      <Select :id="id('day')" v-model="day">
        <SelectItem v-for="entry in DAYS" :key="entry" :value="entry">{{ t('automationWizard.dayOption', { day: entry }) }}</SelectItem>
      </Select>
    </FormField>
    <FormField :for="id('month')" :label="t('salary.forMonth')">
      <Select :id="id('month')" v-model="forMonth">
        <SelectItem v-for="entry in MONTHS" :key="entry" :value="entry">{{ t(`salary.month.${entry}`) }}</SelectItem>
      </Select>
    </FormField>
    <FormField class="col-span-2 sm:col-span-1" :for="id('shift')" :label="t('salary.dayOff')">
      <Select :id="id('shift')" v-model="shift">
        <SelectItem v-for="entry in WORKDAY_SHIFTS" :key="entry" :value="entry">{{ t(`salary.shift.${entry}`) }}</SelectItem>
      </Select>
    </FormField>
    <!-- The mode is wider than half a column: its own row, the usual amount takes the rest. -->
    <div class="col-span-2 grid items-start gap-x-4 gap-y-3 sm:grid-cols-[auto_1fr]">
      <FormField :label="t('salary.amount')">
        <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-fit max-w-full" :model-value="payout.varies ? 'varies' : 'fixed'" @update:model-value="setMode">
          <ToggleGroupItem value="fixed" class="whitespace-nowrap px-3" :data-salary-mode="`${props.idPrefix}-fixed`">{{ t('salary.fixed') }}</ToggleGroupItem>
          <ToggleGroupItem value="varies" class="whitespace-nowrap px-3" :data-salary-mode="`${props.idPrefix}-varies`">{{ t('salary.varies') }}</ToggleGroupItem>
        </ToggleGroup>
      </FormField>
      <FormField
        :for="id('amount')"
        :label="payout.varies ? t('salary.usual') : t('salary.sum')"
        :required="!payout.varies"
        :hint="payout.varies ? t('salary.usualHint') : undefined"
        :error="props.amountError"
      >
        <AmountInput :id="id('amount')" v-model="amount">
          <template #suffix>
            <CurrencyPicker :id="id('currency')" v-model="currency" label="" chip />
          </template>
        </AmountInput>
      </FormField>
    </div>
  </section>
</template>
