<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { drillTo } from '@/shared/lib/drill'
import { formatWeekday } from '@/shared/lib/format'

/** Seven bars with their sums written out: a finger cannot hover for a tooltip. */
const props = defineProps<{ days: SpendingDay[]; currency: string; allowance: Amount | null }>()
const { t } = useI18n()

const max = computed(() => Math.max(1, ...props.days.map((day) => Number(day.daily)), Number(props.allowance ?? 0)))
</script>

<template>
  <ul class="flex h-28 items-end gap-1" :aria-label="t('dashboard.spending.week')">
    <li v-for="day in props.days" :key="day.date" class="flex h-full min-w-0 flex-1">
    <RouterLink
      :to="drillTo({ from: day.date })"
      class="flex h-full min-w-0 flex-1 flex-col items-center gap-0.5 rounded-sm hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
    >
      <span class="max-w-full truncate text-2xs text-muted-foreground tabular-nums">
        <Money :amount="day.daily" :currency="props.currency" :decimals="0" compact :convert="false" />
      </span>
      <span class="flex w-full flex-1 items-end px-0.5">
        <span
          class="w-full rounded-t-sm"
          :class="day.over ? 'bg-destructive' : 'bg-primary'"
          :style="{ height: `${Math.max(2, (Number(day.daily) / max) * 100)}%` }"
        />
      </span>
      <span class="text-2xs text-muted-foreground uppercase">{{ formatWeekday(day.date, 'narrow') }}</span>
    </RouterLink>
    </li>
  </ul>
</template>
