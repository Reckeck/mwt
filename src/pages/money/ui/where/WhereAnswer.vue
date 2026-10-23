<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatMoney } from '@/shared/lib/format'

/** The answer above the table (Money 1 · 6): how much of the income went on spending, and what is left. */
const props = defineProps<{ report: WhereReport }>()

const { t } = useI18n()
const money = (value: string) => formatMoney(value, { decimals: 0, currency: props.report.meta.currency })

const spent = computed(() => props.report.rows.find((row) => row.kind === 'spending')?.value ?? '0')
const left = computed(() => Number(props.report.left))
</script>

<template>
  <section class="grid gap-1.5 rounded-xl border bg-card p-4 shadow-sm" data-tour="where-answer">
    <h2 class="text-xl font-semibold text-balance">
      <span class="max-md:hidden">{{ t('money.where.headline', { income: money(props.report.income), spent: money(spent) }) }}</span>
      <span class="md:hidden">{{ t('money.where.headlineShort', { income: money(props.report.income), spent: money(spent) }) }}</span>
    </h2>
    <p v-if="left > 0" class="text-sm text-muted-foreground">
      <span class="max-md:hidden">{{ t('money.where.left', { left: money(props.report.left) }) }}</span>
      <span class="md:hidden">{{ t('money.where.leftShort', { left: money(props.report.left) }) }}</span>
    </p>
    <p v-else-if="left < 0" class="text-sm text-destructive">{{ t('money.where.overspent', { over: money(String(-left)) }) }}</p>
  </section>
</template>
