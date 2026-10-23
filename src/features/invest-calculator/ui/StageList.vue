<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Plus, X } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Select, SelectItem } from '@/shared/ui/select'

import { FREQS, newStage } from '../model/calc-form'
import type { InvestCalc } from '../model/use-invest-calc'

/**
 * Contribution stages, one row each: the first runs from the start, each next
 * one replaces it from its year — a «step-up» plan (10 000 now, 20 000 from year 3).
 * In the «Взнос» mode the amount is what the calculator looks for.
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
  <fieldset class="grid gap-2" data-tour="calc-stages">
    <div class="flex items-center justify-between gap-2">
      <legend class="text-sm font-medium">{{ t('invest.stages') }}</legend>
      <Button variant="link" size="sm" class="h-auto px-0" @click="addStage">
        <Plus class="size-3.5" />{{ t('invest.addStageShort') }}
      </Button>
    </div>
    <!-- Four in a row while the card is wide enough, else when and how much over how often. -->
    <div
      v-for="(stage, index) in form.stages"
      :key="index"
      class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] items-center gap-2 @md:grid-cols-[5.5rem_minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_auto]"
      data-calc-stage
    >
      <span v-if="index === 0" class="text-sm text-muted-foreground">{{ t('invest.stageStartShort') }}</span>
      <AmountInput
        v-else
        :id="`calc-stage-from-${index}`"
        v-model="stage.fromYear"
        :decimals="0"
        :aria-label="t('invest.stageFrom')"
      >
        <template #suffix><span class="pr-1 text-xs whitespace-nowrap text-muted-foreground">{{ t('invest.stageFromUnit') }}</span></template>
      </AmountInput>
      <p v-if="form.mode === 'contribution'" class="text-sm text-muted-foreground">{{ t('invest.stageSolved') }}</p>
      <AmountInput
        v-else
        :id="`calc-stage-amount-${index}`"
        v-model="stage.amount"
        :decimals="2"
        placeholder="0"
        :aria-label="t('invest.stageAmount')"
        :aria-invalid="index === 0 && Boolean(props.calc.validation.error('stages'))"
      >
        <template #suffix><span class="px-2 text-xs text-muted-foreground uppercase">{{ form.currency }}</span></template>
      </AmountInput>
      <Button
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground @md:order-last"
        :class="index === 0 && 'invisible'"
        :disabled="index === 0"
        :aria-label="t('invest.removeStage')"
        :title="t('invest.removeStage')"
        @click="form.stages.splice(index, 1)"
      >
        <X class="size-4" />
      </Button>
      <Select v-model="stage.freq" :aria-label="t('invest.freq')">
        <SelectItem v-for="freq in FREQS" :key="freq" :value="freq">{{ t(`invest.freqShort.${freq}`) }}</SelectItem>
      </Select>
      <Select v-model="stage.timing" :aria-label="t('invest.timing')">
        <SelectItem value="start">{{ t('invest.timingShort.start') }}</SelectItem>
        <SelectItem value="end">{{ t('invest.timingShort.end') }}</SelectItem>
      </Select>
    </div>
    <p v-if="props.calc.validation.error('stages')" class="text-xs text-destructive">{{ props.calc.validation.error('stages') }}</p>
  </fieldset>
</template>
