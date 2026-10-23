<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { formatMoney } from '@/shared/lib/format'
import { useTxContext } from '@/shared/lib/tx-context'
import { Button } from '@/shared/ui/button'
import { GLANCE_TRIGGER_CLASS } from '@/shared/ui/glance'
import { ProgressBar } from '@/shared/ui/progress-bar'
import { Separator } from '@/shared/ui/separator'

import { glancePayments } from '../lib/payments'
import { useCategoryGlance } from '../model/use-category-glance'
import type { CategoryGlanceProps } from '../quick-glance.types'

/**
 * A category looked at from Ctrl K (Paths 2 · 8): spent this month against
 * the one before and the usual, its share, the last three payments and
 * «Открыть в Тратах →». The host draws it where it has room.
 */
const props = defineProps<CategoryGlanceProps>()
const emit = defineEmits<{ open: [] }>()

const { t } = useI18n()
const context = useTxContext()
const categories = useCategories()
const glance = useCategoryGlance(() => props.category, () => props.month, () => true)

const money = (value: string, currency = glance.currency.value) => formatMoney(value, { decimals: 0, currency })
const label = (key: string) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('money.categories.none'))
const monthIn = (month: string) => t(`period.in.${month.slice(5, 7)}`)
const before = computed(() => {
  const number = Number(props.month.slice(5, 7))
  return t(`period.in.${String(number === 1 ? 12 : number - 1).padStart(2, '0')}`)
})
const facts = computed(() => {
  const parts = [t('glance.spent.before', { month: before.value, sum: money(glance.totalBefore.value) })]
  if (glance.average.value) parts.push(t('glance.category.average', { sum: money(glance.average.value) }))
  const status = glance.limit.value
  parts.push(status ? t('glance.spent.limit', { limit: money(status.budget.limit, status.budget.currency) }) : t('glance.spent.noLimit'))
  return parts.join(' · ')
})
const payments = computed(() => glancePayments(glance.payments.value, { side: context.sideLabel, category: label }))
</script>

<template>
  <div class="flex flex-col gap-2.5 text-sm" data-category-glance>
    <span class="text-xs text-muted-foreground">{{ t('glance.title') }}</span>
    <div class="flex flex-col gap-0.5">
      <span class="text-xs text-muted-foreground">{{ t('home.figures.spent', { when: monthIn(props.month) }) }}</span>
      <span class="text-lg font-semibold tabular-nums">{{ money(glance.total.value) }}</span>
      <span class="text-xs text-muted-foreground">{{ facts }}</span>
    </div>
    <div class="flex flex-col gap-1">
      <div class="flex items-baseline gap-2">
        <span class="min-w-0 flex-1 truncate"><span :class="GLANCE_TRIGGER_CLASS">{{ label(props.category) }}</span></span>
        <b class="tabular-nums">{{ money(glance.total.value) }}</b>
        <span class="w-10 text-right text-muted-foreground tabular-nums">{{ glance.pct.value }} %</span>
      </div>
      <ProgressBar :value="glance.pct.value" size="sm" />
    </div>
    <template v-if="payments.length">
      <Separator />
      <span class="text-xs text-muted-foreground">{{ t('glance.spent.last') }}</span>
      <div v-for="payment in payments" :key="payment.id" class="flex items-baseline gap-2">
        <span class="min-w-0 flex-1 truncate">{{ payment.name }} <span class="text-muted-foreground">· {{ payment.day }}</span></span>
        <b class="tabular-nums">{{ payment.amount }}</b>
      </div>
    </template>
    <Button variant="link" size="sm" class="mt-auto h-auto self-start px-0" data-category-glance-open @click="emit('open')">
      {{ t('glance.spent.open') }} →
    </Button>
  </div>
</template>
