<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'

import { HEAT_CLASSES } from '../spending-calendar.constants'

/** Under the grid: the month's totals, the heat scale and the link to the month balance. */
const props = defineProps<{ currency: string; income: number; spent: number; perDay: string | null }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
    <span>
      <b class="font-semibold text-success">+<Money :amount="String(props.income)" :decimals="0" :currency="props.currency" /></b>
      {{ t('calendar.legend.income') }}
    </span>
    <span>−<Money :amount="String(props.spent)" :decimals="0" :currency="props.currency" /> {{ t('calendar.legend.spent') }}</span>
    <span v-if="props.perDay !== null">
      <Money :amount="props.perDay" :decimals="0" :currency="props.currency" :convert="false" /> — {{ t('calendar.legend.perDay') }}
    </span>
    <span class="inline-flex items-center gap-0.5" aria-hidden="true">
      <span v-for="(heat, index) in HEAT_CLASSES" :key="index" class="size-3 rounded-sm border border-border" :class="heat" />
    </span>
    <RouterLink :to="{ name: 'money-month' }" class="ml-auto text-primary hover:underline">{{ t('calendar.legend.month') }}</RouterLink>
  </div>
</template>
