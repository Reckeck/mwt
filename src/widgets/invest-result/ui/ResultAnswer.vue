<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, MoneyPhrase } from '@/entities/currency'
import type { CalcOutcome } from '@/features/invest-calculator'
import { formatPct, pluralForm } from '@/shared/lib/format'
import { composeSums, type Sum, type SumPart } from '@/shared/lib/money-parts'
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
  /** The card around it turns red itself (a phone): no red frame of its own. */
  unframed?: boolean
}>()
const emit = defineEmits<{ accept: []; goal: []; withdraw: [amount: string] }>()

const { t, locale } = useI18n()
const input = computed(() => props.outcome.input)
const result = computed(() => props.outcome.result)
const solved = computed(() => props.outcome.solved)
const term = computed(() => termText(input.value.term_months, t, locale.value))
const sum = (amount: string | number, decimals = 0): Sum => ({ amount, currency: props.currency, decimals })
/** A phrase whose sums (objects from `sum`) are drawn as `<Money>`. */
function phrase(key: string, values: Record<string, unknown>): SumPart[] {
  const { mark, parts } = composeSums()
  const marked = Object.fromEntries(
    Object.entries(values).map(([name, value]) => [name, typeof value === 'object' && value ? mark(value as Sum) : value]),
  )
  return parts(t(key, marked))
}

const depletedAt = computed(() => (input.value.withdrawal ? result.value.depleted_at : null))
const withdrawal = computed(() => input.value.withdrawal)
/** «Деньги закончатся через 6 мес.» / «in 6 months»: under a year the months take the plural form; longer — «через 2 года 3 мес.». */
const runsOut = computed(() => {
  const months = depletedAt.value ?? 0
  return months < 12
    ? t(`invest.answer.runsOutMonths.${pluralForm(months, locale.value)}`, { n: months })
    : t('invest.answer.runsOut', { term: termText(months, t, locale.value) })
})

/** «это 28 % среднего дохода 1 575 $; сейчас откладываете 157,50 $» — only after «Из данных». */
const incomeShare = computed(() => {
  const income = Number(props.prefilled?.income_avg ?? 0)
  if (solved.value?.unknown !== 'contribution' || !(income > 0)) return null
  const pct = formatPct((Number(solved.value.value) / income) * 100, { decimals: 0 })
  const now = props.prefilled?.monthly_contribution
  return Number(now) > 0
    ? phrase('invest.answer.incomeShareNow', { pct, income: sum(income), now: sum(now ?? 0, 2) })
    : phrase('invest.answer.incomeShare', { pct, income: sum(income) })
})
</script>

<template>
  <!-- A found value stands out as the answer to the question asked (Money 3 · 19). -->
  <!-- Money that runs out is a warning: the answer is framed in red (Money 3 · 21). -->
  <div
    class="grid gap-1"
    :class="[
      solved && 'rounded-lg border border-primary/40 bg-primary/5 p-4',
      depletedAt !== null && withdrawal && !props.unframed && 'rounded-lg border border-destructive/60 p-4',
    ]"
    data-calc-answer
  >
    <template v-if="depletedAt !== null && withdrawal">
      <p class="text-sm text-muted-foreground">
        <MoneyPhrase :parts="phrase('invest.answer.withdrawing', { amount: sum(withdrawal.amount), freq: t(`invest.freqShort.${withdrawal.freq}`) })" />
      </p>
      <p class="text-2xl font-semibold text-destructive max-md:text-xl">
        {{ runsOut }}
      </p>
      <p class="text-sm text-muted-foreground">
        {{ planMonth(input.start, depletedAt) }}
        <template v-if="props.safeWithdrawal !== null">
          · <MoneyPhrase :parts="phrase('invest.answer.safe', { term, amount: sum(props.safeWithdrawal) })" />
        </template>
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
          <MoneyPhrase :parts="phrase('invest.answer.toReach', { date: planMonth(input.start, input.term_months + 1), target: sum(props.target) })" />
        </template>
        <MoneyPhrase v-else-if="solved.unknown === 'term'" :parts="phrase('invest.answer.toReachTerm', { target: sum(props.target) })" />
        <MoneyPhrase v-else :parts="phrase('invest.answer.toReachRate', { term, target: sum(props.target) })" />
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
      <p v-if="incomeShare" class="text-sm text-muted-foreground"><MoneyPhrase :parts="incomeShare" /></p>
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
