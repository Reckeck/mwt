<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Card } from '@/shared/ui/card'
import { FormField } from '@/shared/ui/form-field'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { CAP_FREQS, type CalcGoalMode, GOAL_MODES } from './model/calc-form'
import type { InvestCalc } from './model/use-invest-calc'
import CalcAdvanced from './ui/CalcAdvanced.vue'
import PrefillPanel from './ui/PrefillPanel.vue'
import StageList from './ui/StageList.vue'

/**
 * «Что посчитать»: the fields from the most to the least asked-about. There
 * is no «Рассчитать» — the result follows the fields as they change.
 */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const form = props.calc.form
const error = props.calc.validation.error

function pickMode(value: unknown) {
  if (typeof value === 'string' && (GOAL_MODES as string[]).includes(value)) form.mode = value as CalcGoalMode
}
</script>

<template>
  <Card>
    <form class="@container grid gap-4" novalidate data-calc-form @submit.prevent="props.calc.calculate">
      <div class="grid gap-2">
        <h2 class="text-sm font-medium">{{ t('invest.mode') }}</h2>
        <ToggleGroup
          type="single"
          variant="segmented"
          size="sm"
          :spacing="1"
          class="flex-wrap"
          data-tour="calc-mode"
          :aria-label="t('invest.mode')"
          :model-value="form.mode"
          @update:model-value="pickMode"
        >
          <ToggleGroupItem v-for="mode in GOAL_MODES" :key="mode" :value="mode" :data-calc-mode="mode" :title="t(`invest.modeHint.${mode}`)">
            {{ t(`invest.modeValue.${mode}`) }}
          </ToggleGroupItem>
        </ToggleGroup>
      </div>

      <PrefillPanel :calc="props.calc" />

      <FormField v-if="form.mode !== 'result'" for="calc-target" :label="t('invest.target')" required :error="error('target')">
        <AmountInput id="calc-target" v-model="form.target" :decimals="2" placeholder="0">
          <template #suffix><span class="px-2 text-xs text-muted-foreground uppercase">{{ form.currency }}</span></template>
        </AmountInput>
      </FormField>

      <div class="grid grid-cols-2 gap-3">
        <FormField for="calc-initial" :label="t('invest.initial')" :error="error('initial') ?? error('currency')">
          <AmountInput id="calc-initial" v-model="form.initial" :decimals="2" placeholder="0">
            <template #suffix>
              <CurrencyPicker
                id="calc-currency"
                v-model="form.currency"
                label=""
                compact
                class="w-20 [&_button]:h-7 [&_button]:border-0 [&_button]:bg-transparent [&_button]:px-2 [&_button]:shadow-none"
                :aria-label="t('invest.currency')"
                :error="error('currency')"
              />
            </template>
          </AmountInput>
        </FormField>
        <FormField v-if="form.mode !== 'rate'" for="calc-rate" :label="t('invest.rateShort')" :error="error('rate')">
          <AmountInput id="calc-rate" v-model="form.rate" :decimals="4" placeholder="0" :title="t('invest.rateHint')">
            <template #suffix><span class="px-2 text-xs whitespace-nowrap text-muted-foreground">{{ t('invest.rateUnit') }}</span></template>
          </AmountInput>
        </FormField>
        <FormField
          v-if="form.mode !== 'term'"
          for="calc-years"
          :label="t('invest.term')"
          :error="error('years') ?? error('months')"
          class="@max-sm:col-span-2"
        >
          <div class="grid grid-cols-2 gap-2">
            <AmountInput id="calc-years" v-model="form.years" :decimals="0" placeholder="0">
              <template #suffix><span class="px-2 text-xs text-muted-foreground">{{ t('invest.yearsUnit') }}</span></template>
            </AmountInput>
            <AmountInput id="calc-months" v-model="form.months" :decimals="0" placeholder="0" :aria-label="t('invest.months')">
              <template #suffix><span class="px-2 text-xs text-muted-foreground">{{ t('invest.monthsUnit') }}</span></template>
            </AmountInput>
          </div>
        </FormField>
        <FormField for="calc-cap" :label="t('invest.capitalization')" :class="form.mode === 'term' && 'col-span-2'">
          <Select id="calc-cap" v-model="form.capitalization" :title="t('invest.capitalizationHint')">
            <SelectItem v-for="freq in CAP_FREQS" :key="freq" :value="freq">{{ t(`invest.capValue.${freq}`) }}</SelectItem>
          </Select>
        </FormField>
      </div>

      <StageList :calc="props.calc" />
      <CalcAdvanced :calc="props.calc" />
    </form>
  </Card>
</template>
