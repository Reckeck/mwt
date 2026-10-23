<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import type { CalcOutcome } from '@/features/invest-calculator'
import { formatMoney, formatPct } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import { planMonth, termText } from '../lib/answer'

/**
 * The answer as the first line, in the words of the question: «Через 10 лет
 * будет…», «Чтобы к окт 2029 накопить… нужно вносить…», and when withdrawals
 * eat the money — «закончатся через…» with the amount that lasts.
 */
const props = defineProps<{
  outcome: CalcOutcome
  currency: string
  target: string
  prefilled: ScenarioPrefill | null
  safeWithdrawal: string | null
}>()
const emit = defineEmits<{ accept: []; goal: []; withdraw: [amount: string] }>()

const { t, locale } = useI18n()
const input = computed(() => props.outcome.input)
const result = computed(() => props.outcome.result)
const solved = computed(() => props.outcome.solved)
const term = computed(() => termText(input.value.term_months, t, locale.value))
const money = (amount: string | number) => formatMoney(amount, { currency: props.currency, decimals: 0 })
const exact = (amount: string | number) => formatMoney(amount, { currency: props.currency, decimals: 2 })

const depletedAt = computed(() => (input.value.withdrawal ? result.value.depleted_at : null))
const withdrawal = computed(() => input.value.withdrawal)

/** «это 28 % среднего дохода 1 575 $; сейчас откладываете 157,50 $» — only after «Из данных». */
const incomeShare = computed(() => {
  const income = Number(props.prefilled?.income_avg ?? 0)
  if (solved.value?.unknown !== 'contribution' || !(income > 0)) return null
  const pct = formatPct((Number(solved.value.value) / income) * 100, { decimals: 0 })
  const now = props.prefilled?.monthly_contribution
  return now
    ? t('invest.answer.incomeShareNow', { pct, income: money(income), now: exact(now) })
    : t('invest.answer.incomeShare', { pct, income: money(income) })
})
</script>

<template>
  <!-- A found value stands out as the answer to the question asked (Money 3 · 19). -->
  <div class="grid gap-1" :class="solved && 'rounded-lg border border-primary/40 bg-primary/5 p-4'" data-calc-answer>
    <template v-if="depletedAt !== null && withdrawal">
      <p class="text-sm text-muted-foreground">
        {{ t('invest.answer.withdrawing', { amount: money(withdrawal.amount), freq: t(`invest.freqShort.${withdrawal.freq}`) }) }}
      </p>
      <p class="text-2xl font-semibold text-destructive">
        {{ t('invest.answer.runsOut', { term: termText(depletedAt, t, locale) }) }}
      </p>
      <p class="text-sm text-muted-foreground">
        {{ planMonth(input.start, depletedAt) }}
        <template v-if="props.safeWithdrawal !== null">· {{ t('invest.answer.safe', { term, amount: money(props.safeWithdrawal) }) }}</template>
        <Button
          v-if="props.safeWithdrawal !== null"
          variant="link"
          size="sm"
          class="h-auto px-1"
          @click="emit('withdraw', props.safeWithdrawal)"
        >
          {{ t('invest.answer.apply') }}
        </Button>
      </p>
    </template>

    <template v-else-if="solved">
      <p class="text-sm text-muted-foreground">
        <template v-if="solved.unknown === 'contribution'">
          {{ t('invest.answer.toReach', { date: planMonth(input.start, input.term_months + 1), target: money(props.target) }) }}
        </template>
        <template v-else-if="solved.unknown === 'term'">{{ t('invest.answer.toReachTerm', { target: money(props.target) }) }}</template>
        <template v-else>{{ t('invest.answer.toReachRate', { term, target: money(props.target) }) }}</template>
      </p>
      <p class="text-2xl font-semibold" :class="!solved.reachable && 'text-warning'">
        <template v-if="!solved.reachable">{{ t(`invest.unreachable.${solved.unknown}`) }} </template>
        <template v-if="solved.unknown === 'contribution'">
          {{ t('invest.answer.contribution') }} <Money :amount="solved.value" :currency="props.currency" :decimals="2" />
          <span class="ms-1 text-base font-normal text-muted-foreground">{{ t('invest.answer.perMonth') }}</span>
        </template>
        <template v-else-if="solved.unknown === 'term'">
          {{ t('invest.answer.term', { term: termText(Number(solved.value), t, locale) }) }}
        </template>
        <template v-else>{{ t('invest.answer.rate', { pct: formatPct(solved.value) }) }}</template>
      </p>
      <p v-if="incomeShare" class="text-sm text-muted-foreground">{{ incomeShare }}</p>
      <div v-if="solved.reachable" class="flex flex-wrap gap-2 pt-2">
        <Button size="sm" @click="emit('goal')">{{ t(`invest.answer.goal.${solved.unknown}`) }}</Button>
        <Button variant="outline" size="sm" @click="emit('accept')">{{ t('invest.solved.apply') }}</Button>
      </div>
    </template>

    <template v-else>
      <p class="text-sm text-muted-foreground">{{ t('invest.answer.after', { term }) }}</p>
      <p class="text-3xl font-semibold tracking-tight"><Money :amount="result.final_nominal" :currency="props.currency" :decimals="0" /></p>
      <p class="text-sm">
        {{ t('invest.answer.real') }} <b><Money :amount="result.final_real" :currency="props.currency" :decimals="0" /></b>
      </p>
    </template>
  </div>
</template>
