<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { groupByDay } from '@/entities/transaction'
import { pluralForm } from '@/shared/lib/format'
import { useTxContext } from '@/shared/lib/tx-context'

/**
 * `3 operations · 36 $`: how many operations the filter keeps and, once every page of them is
 * loaded, their net in the base currency.
 */
const props = defineProps<{
  rows: Transaction[]
  total: number
  /** The net is shown only for a narrowed list. */
  narrowed: boolean
  fxPerUsd?: string | number
  baseCurrency?: string
}>()

const { t, locale } = useI18n()
const context = useTxContext()

const count = computed(() => t(`filter.operations.${pluralForm(props.total, locale.value)}`, { count: props.total }))
const net = computed(() => {
  if (!props.narrowed || !props.baseCurrency || props.rows.length < props.total) return null
  const days = groupByDay(props.rows, Number(props.fxPerUsd ?? 1), context.rootOf)
  return days.reduce((sum, day) => sum + day.totalBase, 0)
})
</script>

<template>
  <span class="text-sm text-muted-foreground tabular-nums" data-filter-total>
    {{ count }}
    <template v-if="net !== null">
      · <b class="font-semibold text-foreground"><Money :amount="net" :currency="props.baseCurrency" signed /></b>
    </template>
  </span>
</template>
