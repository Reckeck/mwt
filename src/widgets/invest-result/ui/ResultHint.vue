<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import type { CalcOutcome } from '@/features/invest-calculator'
import { formatMoney, formatPct } from '@/shared/lib/format'

import { termText } from '../lib/answer'
import type { ContributionAlternative } from '../model/use-answer-extras'

/** «Если взнос не по силам: срок 5 лет — 239 $ в месяц, или доходность 10 % — 434 $». */
const props = defineProps<{
  solved: NonNullable<CalcOutcome['solved']>
  currency: string
  alternatives: ContributionAlternative[]
}>()

const { t, locale } = useI18n()

const hint = computed(() => {
  const [longer, richer] = props.alternatives
  // A contribution of zero is already the easiest: nothing to soften.
  if (!longer || !richer || !(Number(props.solved.value) > 0)) return null
  const money = (amount: string) => formatMoney(amount, { currency: props.currency, decimals: 0 })
  return t('invest.answer.alternatives', {
    term: termText(longer.at, t, locale.value),
    termAmount: money(longer.contribution),
    rate: formatPct(richer.at, { decimals: 0 }),
    rateAmount: money(richer.contribution),
  })
})
</script>

<template>
  <p v-if="hint" class="flex gap-2 text-sm text-muted-foreground" data-calc-hint><span class="text-warning">●</span>{{ hint }}</p>
</template>
