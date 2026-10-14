<script setup lang="ts">
import { computed } from 'vue'

import { formatPct, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import { Money } from '../money'
import type { PnlTextProps } from './pnl-text.types'

/** Profit or loss: always signed and coloured by direction; zero stays neutral. */
const props = withDefaults(defineProps<PnlTextProps>(), {
  currency: '',
  decimals: undefined,
  pct: undefined,
})

const percent = computed(() =>
  props.pct === null || props.pct === undefined || props.pct === '' ? '' : formatPct(props.pct, { signed: true }),
)
</script>

<template>
  <span class="inline-flex items-baseline gap-[0.35em] whitespace-nowrap">
    <Money :amount="props.amount" :currency="props.currency" :decimals="props.decimals" colored />
    <span v-if="percent" :class="cn('text-[0.85em] tabular-nums', signClass(props.amount))">({{ percent }})</span>
  </span>
</template>
