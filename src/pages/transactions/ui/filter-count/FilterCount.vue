<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDay, formatMonthKey, pluralForm } from '@/shared/lib/format'

import type { FilterCountProps } from './filter-count.types'

/** «5 операций за октябрь»: how many the filter keeps, and over what. */
const props = defineProps<FilterCountProps>()

const { t, locale } = useI18n()

/** One whole month reads as its name; other days as «1 окт – 15 окт». */
const period = computed(() => {
  const { from, to } = props.filter
  if (!from && !to) return ''
  if (from && to && from.slice(0, 7) === to.slice(0, 7) && from.endsWith('-01')) {
    const last = new Date(Number(to.slice(0, 4)), Number(to.slice(5, 7)), 0).getDate()
    if (Number(to.slice(8)) === last) return formatMonthKey(from, { month: 'long' })
  }
  return `${formatDay(from)} – ${formatDay(to)}`
})
const count = computed(() => t(`filter.operations.${pluralForm(props.total, locale.value)}`, { count: props.total }))
</script>

<template>
  <span class="text-sm text-muted-foreground tabular-nums">
    {{ period ? t('filter.countOver', { count, period }) : count }}
  </span>
</template>
