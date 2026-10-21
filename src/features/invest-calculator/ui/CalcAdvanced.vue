<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { FREQS } from '../model/calc-form'
import type { InvestCalc } from '../model/use-invest-calc'

/** Tax, inflation, indexation and withdrawals: folded, most runs leave them as they are. */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const form = props.calc.form
const error = props.calc.validation.error

/** Opens by itself when a folded field holds something or a check points at one. */
const opened = computed(
  () =>
    Number(form.taxPct) > 0 ||
    Number(form.indexation) > 0 ||
    form.withdrawal.on ||
    Boolean(error('taxPct') || error('inflation') || error('indexation') || error('withdrawal')),
)

function pickTaxMode(value: unknown) {
  if (value === 'yearly' || value === 'at_end') form.taxMode = value
}
</script>

<template>
  <AdvancedFields :label="t('invest.advanced')" :default-open="opened">
    <div class="grid gap-3 @md:grid-cols-2">
      <FormField for="calc-indexation" :label="t('invest.indexation')" :hint="t('invest.indexationHint')" :error="error('indexation')">
        <AmountInput id="calc-indexation" v-model="form.indexation" :decimals="2" placeholder="0" />
      </FormField>
      <FormField for="calc-inflation" :label="t('invest.inflation')" :hint="t('invest.inflationHint')" :error="error('inflation')">
        <AmountInput id="calc-inflation" v-model="form.inflation" :decimals="2" placeholder="0" />
      </FormField>
      <FormField for="calc-tax" :label="t('invest.tax')" :hint="t('invest.taxHint')" :error="error('taxPct')">
        <AmountInput id="calc-tax" v-model="form.taxPct" :decimals="2" placeholder="0" />
      </FormField>
      <FormField :label="t('invest.taxMode')">
        <ToggleGroup type="single" variant="outline" size="sm" :model-value="form.taxMode" @update:model-value="pickTaxMode">
          <ToggleGroupItem value="yearly">{{ t('invest.taxModeValue.yearly') }}</ToggleGroupItem>
          <ToggleGroupItem value="at_end">{{ t('invest.taxModeValue.at_end') }}</ToggleGroupItem>
        </ToggleGroup>
      </FormField>
    </div>

    <SwitchRow id="calc-withdrawal-on" v-model="form.withdrawal.on" :label="t('invest.withdrawal')" :hint="t('invest.withdrawalHint')" />
    <div v-if="form.withdrawal.on" class="grid gap-3 @md:grid-cols-3">
      <FormField for="calc-withdrawal" :label="t('invest.withdrawalAmount')" required :error="error('withdrawal')">
        <AmountInput id="calc-withdrawal" v-model="form.withdrawal.amount" :decimals="2" placeholder="0" />
      </FormField>
      <FormField :label="t('invest.freq')">
        <Select v-model="form.withdrawal.freq">
          <SelectItem v-for="freq in FREQS" :key="freq" :value="freq">{{ t(`invest.freqValue.${freq}`) }}</SelectItem>
        </Select>
      </FormField>
      <FormField for="calc-withdrawal-from" :label="t('invest.stageFrom')">
        <Input id="calc-withdrawal-from" v-model="form.withdrawal.fromYear" inputmode="numeric" />
      </FormField>
      <SwitchRow
        id="calc-withdrawal-indexed"
        v-model="form.withdrawal.indexed"
        class="@md:col-span-3"
        :label="t('invest.withdrawalIndexed')"
      />
    </div>
  </AdvancedFields>
</template>
