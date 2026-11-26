<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { advancedSummary } from '../lib/advanced-summary'
import { FREQS } from '../model/calc-form'
import type { InvestCalc } from '../model/use-invest-calc'

/**
 * Tax, inflation, indexation and withdrawals: folded into one row that says
 * what they hold now (`inflation 3.74 % · no tax · no withdrawals`).
 */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const form = props.calc.form
const error = props.calc.validation.error
const summary = computed(() => advancedSummary(form, t))

const wrong = computed(() => Boolean(error('taxPct') || error('inflation') || error('indexation') || error('withdrawal')))
/** Opens by itself for withdrawals (they change the answer) and when a check points inside. */
const open = ref(form.withdrawal.on || wrong.value)
watch([() => form.withdrawal.on, wrong], ([on, bad]) => {
  if (on || bad) open.value = true
})

function pickTaxMode(value: unknown) {
  if (value === 'yearly' || value === 'at_end') form.taxMode = value
}
</script>

<template>
  <Collapsible v-model:open="open" class="grid gap-3 border-t pt-3" data-calc-advanced>
    <CollapsibleTrigger
      class="touch-hit flex w-full items-center justify-between gap-3 rounded-md text-left focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
    >
      <span class="grid min-w-0 gap-0.5">
        <span class="text-sm font-medium">{{ t('invest.advanced') }}</span>
        <span class="truncate text-xs text-muted-foreground"><template v-for="(part, at) in summary" :key="at">
            <Money v-if="'amount' in part" v-bind="part" />
            <template v-else>{{ part.text }}</template>
          </template>
        </span>
      </span>
      <ChevronRight class="size-4 shrink-0 text-muted-foreground transition-transform" :class="open && 'rotate-90'" />
    </CollapsibleTrigger>
    <CollapsibleContent class="grid gap-3">
      <div class="grid grid-cols-3 gap-3">
        <FormField for="calc-indexation" :label="t('invest.indexationShort')" :error="error('indexation')">
          <AmountInput id="calc-indexation" v-model="form.indexation" :decimals="2" placeholder="0" :title="t('invest.indexationHint')">
            <template #suffix><span class="px-2 text-xs whitespace-nowrap text-muted-foreground">{{ t('invest.perYearUnit') }}</span></template>
          </AmountInput>
        </FormField>
        <FormField for="calc-inflation" :label="t('invest.inflationShort')" :error="error('inflation')">
          <AmountInput id="calc-inflation" v-model="form.inflation" :decimals="2" placeholder="0" :title="t('invest.inflationHint')">
            <template #suffix><span class="px-2 text-xs text-muted-foreground">%</span></template>
          </AmountInput>
        </FormField>
        <FormField for="calc-tax" :label="t('invest.taxShort')" :error="error('taxPct')">
          <AmountInput id="calc-tax" v-model="form.taxPct" :decimals="2" placeholder="0" :title="t('invest.taxHint')">
            <template #suffix><span class="px-2 text-xs text-muted-foreground">%</span></template>
          </AmountInput>
        </FormField>
      </div>
      <FormField v-if="Number(form.taxPct) > 0" :label="t('invest.taxMode')">
        <ToggleGroup type="single" variant="outline" size="sm" :model-value="form.taxMode" @update:model-value="pickTaxMode">
          <ToggleGroupItem value="yearly">{{ t('invest.taxModeValue.yearly') }}</ToggleGroupItem>
          <ToggleGroupItem value="at_end">{{ t('invest.taxModeValue.at_end') }}</ToggleGroupItem>
        </ToggleGroup>
      </FormField>

      <SwitchRow id="calc-withdrawal-on" v-model="form.withdrawal.on" :label="t('invest.withdrawal')" :hint="t('invest.withdrawalHint')" />
      <div v-if="form.withdrawal.on" class="grid grid-cols-2 gap-3 @md:grid-cols-3">
        <FormField for="calc-withdrawal" :label="t('invest.stageAmount')" required :error="error('withdrawal')">
          <AmountInput id="calc-withdrawal" v-model="form.withdrawal.amount" :decimals="2" placeholder="0">
            <template #suffix><span class="px-2 text-xs text-muted-foreground uppercase">{{ form.currency }}</span></template>
          </AmountInput>
        </FormField>
        <FormField for="calc-withdrawal-from" :label="t('invest.stageFrom')">
          <AmountInput id="calc-withdrawal-from" v-model="form.withdrawal.fromYear" :decimals="0" />
        </FormField>
        <FormField :label="t('invest.freq')" class="@max-md:col-span-2">
          <Select v-model="form.withdrawal.freq">
            <SelectItem v-for="freq in FREQS" :key="freq" :value="freq">{{ t(`invest.freqValue.${freq}`) }}</SelectItem>
          </Select>
        </FormField>
        <SwitchRow
          id="calc-withdrawal-indexed"
          v-model="form.withdrawal.indexed"
          plain
          class="col-span-2 @md:col-span-3"
          :label="t('invest.withdrawalIndexed')"
        />
      </div>
    </CollapsibleContent>
  </Collapsible>
</template>
