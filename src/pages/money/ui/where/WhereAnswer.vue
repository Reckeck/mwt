<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { MoneyPhrase } from '@/entities/currency'
import { composeSums, type SumPart } from '@/shared/lib/money-parts'

/** The answer above the table (Money 1 · 6): how much of the income went on spending, and what is left. */
const props = defineProps<{ report: WhereReport }>()

const { t } = useI18n()

const spent = computed(() => props.report.rows.find((row) => row.kind === 'spending')?.value ?? '0')
const left = computed(() => Number(props.report.left))

/** A phrase of the answer with its sums named `income`, `spent`, `left`, `over`. */
function phrase(key: string, sums: Record<string, string>): SumPart[] {
  const { mark, parts } = composeSums()
  const marked = Object.fromEntries(
    Object.entries(sums).map(([name, amount]) => [name, mark({ amount, currency: props.report.meta.currency, decimals: 0 })]),
  )
  return parts(t(key, marked))
}
const sums = computed(() => ({ income: props.report.income, spent: spent.value }))
</script>

<template>
  <section class="grid gap-1.5 rounded-xl border bg-card p-4 shadow-sm" data-tour="where-answer">
    <h2 class="text-xl font-semibold text-balance">
      <span class="max-md:hidden"><MoneyPhrase :parts="phrase('money.where.headline', sums)" /></span>
      <span class="md:hidden"><MoneyPhrase :parts="phrase('money.where.headlineShort', sums)" /></span>
    </h2>
    <p v-if="left > 0" class="text-sm text-muted-foreground">
      <span class="max-md:hidden"><MoneyPhrase :parts="phrase('money.where.left', { left: props.report.left })" /></span>
      <span class="md:hidden"><MoneyPhrase :parts="phrase('money.where.leftShort', { left: props.report.left })" /></span>
    </p>
    <p v-else-if="left < 0" class="text-sm text-destructive">
      <MoneyPhrase :parts="phrase('money.where.overspent', { over: String(-left) })" />
    </p>
  </section>
</template>
