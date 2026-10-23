<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import type { CalcOutcome } from '@/features/invest-calculator'
import { formatPct } from '@/shared/lib/format'

import { investedFormula, planMonth, termText } from '../lib/answer'

/**
 * Three figures under the answer, each with how it came about: what went in
 * («100 000 + 10 000 × 120»), what it earned, and the third that fits the
 * question — the effective rate, the total by the date, or what is left.
 */
const props = defineProps<{ outcome: CalcOutcome; currency: string }>()

const { t, locale } = useI18n()
const input = computed(() => props.outcome.input)
const result = computed(() => props.outcome.result)
const withdrawing = computed(() => Number(result.value.withdrawn) > 0)
const lastMonth = computed(() => result.value.depleted_at ?? input.value.term_months)
const earnedMore = computed(() => Number(result.value.interest) > Number(result.value.invested))
</script>

<template>
  <dl class="grid grid-cols-3 gap-4 border-y py-3" data-calc-stats>
    <div v-if="withdrawing" class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.withdrawn') }}</dt>
      <dd class="text-base font-semibold tabular-nums"><Money :amount="result.withdrawn" :currency="props.currency" :decimals="0" /></dd>
      <dd class="text-xs text-muted-foreground">{{ t('invest.stat.during', { term: termText(lastMonth, t, locale) }) }}</dd>
    </div>
    <div v-else class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.invested') }}</dt>
      <dd class="text-base font-semibold tabular-nums"><Money :amount="result.invested" :currency="props.currency" :decimals="0" /></dd>
      <dd v-if="investedFormula(input)" class="truncate text-xs text-muted-foreground tabular-nums">{{ investedFormula(input) }}</dd>
    </div>

    <div class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.income') }}</dt>
      <dd class="text-base font-semibold tabular-nums"><Money :amount="result.interest" :currency="props.currency" :decimals="0" /></dd>
      <dd v-if="!withdrawing && !props.outcome.solved" class="text-xs text-muted-foreground">
        {{ t(earnedMore ? 'invest.stat.moreThanInvested' : 'invest.stat.lessThanInvested') }}
      </dd>
    </div>

    <div v-if="withdrawing" class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.left') }}</dt>
      <dd class="text-base font-semibold tabular-nums"><Money :amount="result.final_nominal" :currency="props.currency" :decimals="0" /></dd>
      <dd class="text-xs text-muted-foreground">{{ planMonth(input.start, lastMonth) }}</dd>
    </div>
    <div v-else-if="props.outcome.solved" class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.final') }}</dt>
      <dd class="text-base font-semibold tabular-nums"><Money :amount="result.final_nominal" :currency="props.currency" :decimals="0" /></dd>
      <dd class="text-xs text-muted-foreground">{{ t('invest.stat.by', { date: planMonth(input.start, input.term_months + 1) }) }}</dd>
    </div>
    <div v-else class="grid min-w-0 gap-0.5">
      <dt class="text-xs text-muted-foreground">{{ t('invest.stat.effectiveRate') }}</dt>
      <dd class="text-base font-semibold tabular-nums">{{ formatPct(result.effective_rate_pct, { decimals: 2 }) }}</dd>
      <dd class="text-xs text-muted-foreground">{{ t('invest.stat.capitalized', { freq: t(`invest.capValue.${input.capitalization}`).toLowerCase() }) }}</dd>
    </div>
  </dl>
</template>
