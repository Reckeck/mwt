<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
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

const label = (key: string) => (key ? categoryKeyLabel(key, categories.data.value, t) : t('money.categories.none'))
const monthIn = (month: string) => t(`period.in.${month.slice(5, 7)}`)
const before = computed(() => {
  const number = Number(props.month.slice(5, 7))
  return t(`period.in.${String(number === 1 ? 12 : number - 1).padStart(2, '0')}`)
})
const payments = computed(() => glancePayments(glance.payments.value, { side: context.sideLabel, category: label }))
</script>

<template>
  <div class="flex flex-col gap-3 text-sm" data-category-glance>
    <span class="text-xs text-muted-foreground">{{ t('glance.title') }}</span>
    <div class="flex flex-col gap-0.5">
      <span class="text-xs text-muted-foreground">{{ t('home.figures.spent', { when: monthIn(props.month) }) }}</span>
      <span class="text-lg font-semibold tabular-nums"><Money :amount="glance.total.value" :currency="glance.currency.value" :decimals="0" /></span>
      <span class="text-xs text-muted-foreground">
        <i18n-t keypath="glance.spent.before">
          <template #month>{{ before }}</template>
          <template #sum><Money :amount="glance.totalBefore.value" :currency="glance.currency.value" :decimals="0" /></template>
        </i18n-t>
        <template v-if="glance.average.value">
          ·
          <i18n-t keypath="glance.category.average">
            <template #sum><Money :amount="glance.average.value" :currency="glance.currency.value" :decimals="0" /></template>
          </i18n-t>
        </template>
        ·
        <i18n-t v-if="glance.limit.value" keypath="glance.spent.limit">
          <template #limit>
            <Money :amount="glance.limit.value.budget.limit" :currency="glance.limit.value.budget.currency" :decimals="0" />
          </template>
        </i18n-t>
        <template v-else>{{ t('glance.spent.noLimit') }}</template>
      </span>
    </div>
    <div class="flex flex-col gap-1">
      <div class="flex items-baseline gap-2">
        <span class="min-w-0 flex-1 truncate"><span :class="GLANCE_TRIGGER_CLASS">{{ label(props.category) }}</span></span>
        <b class="tabular-nums"><Money :amount="glance.total.value" :currency="glance.currency.value" :decimals="0" /></b>
        <span class="w-10 text-right text-muted-foreground tabular-nums">{{ glance.pct.value }} %</span>
      </div>
      <ProgressBar :value="glance.pct.value" size="sm" />
    </div>
    <template v-if="payments.length">
      <Separator />
      <span class="text-xs text-muted-foreground">{{ t('glance.spent.last') }}</span>
      <div v-for="payment in payments" :key="payment.id" class="flex items-baseline gap-2">
        <span class="min-w-0 flex-1 truncate">{{ payment.name }} <span class="text-muted-foreground">· {{ payment.day }}</span></span>
        <b class="tabular-nums"><Money :amount="payment.amount" :currency="payment.currency" :at="payment.at" :decimals="0" /></b>
      </div>
    </template>
    <Button variant="link" size="sm" class="mt-auto h-auto self-start px-0" data-category-glance-open @click="emit('open')">
      {{ t('glance.spent.open') }} →
    </Button>
  </div>
</template>
