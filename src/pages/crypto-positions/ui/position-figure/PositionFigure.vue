<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'

import type { PositionFigureProps } from './position-figure.types'

/**
 * One figure of a position: the amount through `<Money>`, with `≈` beside it when the adapter says it is an
 * approximation, or a muted dash with the reason when nobody knows it. The dash is never a zero.
 */
const props = withDefaults(defineProps<PositionFigureProps>(), {
  amount: undefined,
  approximate: false,
  colored: false,
  decimals: 2,
  reason: undefined,
  tokens: '',
})

const { t } = useI18n()
const missing = computed(() => props.amount === null || props.amount === undefined || props.amount === '')
const reasonText = computed(() =>
  props.reason ? t(`cryptoTrack.positions.reason.${props.reason}`, { tokens: props.tokens }) : t('cryptoTrack.positions.reason.unknown'),
)
</script>

<template>
  <span v-if="missing" class="text-muted-foreground" :title="reasonText" :aria-label="reasonText">—</span>
  <span v-else class="inline-flex items-baseline gap-[0.25em]">
    <span v-if="props.approximate" class="text-muted-foreground" :title="t('cryptoTrack.positions.approximateMark')">≈</span>
    <Money :amount="props.amount" currency="usd" :decimals="props.decimals" :colored="props.colored" />
  </span>
</template>
