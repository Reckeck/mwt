<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { CAP_FREQS, type CalcGoalMode, GOAL_MODES } from './model/calc-form'
import type { InvestCalc } from './model/use-invest-calc'
import CalcAdvanced from './ui/CalcAdvanced.vue'
import PrefillPanel from './ui/PrefillPanel.vue'
import StageList from './ui/StageList.vue'

/** The calculator's fields; the result follows them live, the button only shows what is wrong. */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const form = props.calc.form
const error = props.calc.validation.error

function pickMode(value: unknown) {
  if (typeof value === 'string' && (GOAL_MODES as string[]).includes(value)) form.mode = value as CalcGoalMode
}
</script>

<template>
  <form class="@container grid gap-4" novalidate @submit.prevent="props.calc.calculate">
    <FormField :label="t('invest.mode')" :hint="t(`invest.modeHint.${form.mode}`)">
      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        class="flex-wrap"
        data-tour="calc-mode"
        :model-value="form.mode"
        @update:model-value="pickMode"
      >
        <ToggleGroupItem v-for="mode in GOAL_MODES" :key="mode" :value="mode">{{ t(`invest.modeValue.${mode}`) }}</ToggleGroupItem>
      </ToggleGroup>
    </FormField>

    <div class="grid gap-3 @md:grid-cols-2">
      <FormField v-if="form.mode !== 'result'" for="calc-target" :label="t('invest.target')" required :error="error('target')">
        <AmountInput id="calc-target" v-model="form.target" :decimals="2" placeholder="0" />
      </FormField>
      <FormField for="calc-currency" :label="t('invest.currency')" required :error="error('currency')">
        <CurrencyPicker id="calc-currency" v-model="form.currency" :error="error('currency')" />
      </FormField>
      <FormField for="calc-initial" :label="t('invest.initial')" :error="error('initial')">
        <AmountInput id="calc-initial" v-model="form.initial" :decimals="2" placeholder="0" />
      </FormField>
      <FormField
        v-if="form.mode !== 'rate'"
        for="calc-rate"
        :label="t('invest.rate')"
        :hint="t('invest.rateHint')"
        :error="error('rate')"
      >
        <AmountInput id="calc-rate" v-model="form.rate" :decimals="4" placeholder="0" />
      </FormField>
      <div v-if="form.mode !== 'term'" class="grid grid-cols-2 gap-2">
        <FormField for="calc-years" :label="t('invest.years')" :error="error('years')">
          <Input id="calc-years" v-model="form.years" inputmode="numeric" />
        </FormField>
        <FormField for="calc-months" :label="t('invest.months')" :error="error('months')">
          <Input id="calc-months" v-model="form.months" inputmode="numeric" />
        </FormField>
      </div>
      <FormField for="calc-cap" :label="t('invest.capitalization')" :hint="t('invest.capitalizationHint')">
        <Select id="calc-cap" v-model="form.capitalization">
          <SelectItem v-for="freq in CAP_FREQS" :key="freq" :value="freq">{{ t(`invest.capValue.${freq}`) }}</SelectItem>
        </Select>
      </FormField>
    </div>

    <StageList :calc="props.calc" />
    <PrefillPanel :calc="props.calc" />
    <CalcAdvanced :calc="props.calc" />

    <Button type="submit" class="w-fit">{{ t('invest.calculate') }}</Button>
  </form>
</template>
