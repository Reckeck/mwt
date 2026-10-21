<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus, X } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import { FREQS, newStage } from '../model/calc-form'
import type { InvestCalc } from '../model/use-invest-calc'

/**
 * Contribution stages: the first runs from the start, each next one replaces
 * it from its year — a «step-up» plan (10 000 now, 20 000 from year 3).
 */
const props = defineProps<{ calc: InvestCalc }>()

const { t } = useI18n()
const form = props.calc.form

function addStage() {
  const last = form.stages.at(-1)
  const nextYear = String(Math.max(2, Number(last?.fromYear ?? 1) + 2))
  form.stages.push({ ...newStage(nextYear), amount: last?.amount ?? '' })
}
</script>

<template>
  <fieldset class="grid gap-3" data-tour="calc-stages">
    <legend class="mb-1 text-sm font-medium">{{ t('invest.stages') }}</legend>
    <div
      v-for="(stage, index) in form.stages"
      :key="index"
      class="grid grid-cols-2 items-end gap-2 @md:grid-cols-[6rem_minmax(0,1fr)_9rem_9rem_2.5rem]"
    >
      <FormField :for="`calc-stage-from-${index}`" :label="t('invest.stageFrom')">
        <Input
          v-if="index > 0"
          :id="`calc-stage-from-${index}`"
          v-model="stage.fromYear"
          inputmode="numeric"
          :aria-label="t('invest.stageFrom')"
        />
        <p v-else class="flex h-9 items-center text-sm text-muted-foreground">{{ t('invest.stageStart') }}</p>
      </FormField>
      <FormField
        :for="`calc-stage-amount-${index}`"
        :label="t('invest.stageAmount')"
        :error="index === 0 ? props.calc.validation.error('stages') : undefined"
      >
        <AmountInput :id="`calc-stage-amount-${index}`" v-model="stage.amount" :decimals="2" placeholder="0" />
      </FormField>
      <FormField :label="t('invest.freq')">
        <Select v-model="stage.freq">
          <SelectItem v-for="freq in FREQS" :key="freq" :value="freq">{{ t(`invest.freqValue.${freq}`) }}</SelectItem>
        </Select>
      </FormField>
      <FormField :label="t('invest.timing')">
        <Select v-model="stage.timing">
          <SelectItem value="start">{{ t('invest.timingValue.start') }}</SelectItem>
          <SelectItem value="end">{{ t('invest.timingValue.end') }}</SelectItem>
        </Select>
      </FormField>
      <Button
        variant="ghost"
        size="icon-sm"
        class="mb-0.5 text-muted-foreground"
        :class="index === 0 && 'invisible'"
        :disabled="index === 0"
        :aria-label="t('invest.removeStage')"
        :title="t('invest.removeStage')"
        @click="form.stages.splice(index, 1)"
      >
        <X class="size-4" />
      </Button>
    </div>
    <Button variant="outline" size="sm" class="w-fit" @click="addStage">
      <Plus class="size-4" />{{ t('invest.addStage') }}
    </Button>
  </fieldset>
</template>
