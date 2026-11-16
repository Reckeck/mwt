<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatPct } from '@/shared/lib/format'

/** The annual rate. It comes as a fraction (`0.125` is 12,5 %); absent means unknown, and prints a dash. */
const props = defineProps<{ apr?: string; approximate?: boolean }>()
const { t } = useI18n()
const percent = computed(() => (props.apr === undefined || props.apr === '' ? null : Number(props.apr) * 100))
</script>

<template>
  <span v-if="percent === null" class="text-muted-foreground" :title="t('cryptoTrack.positions.reason.noRate')">—</span>
  <span v-else class="inline-flex items-baseline gap-[0.25em] whitespace-nowrap tabular-nums">
    <span v-if="props.approximate" class="text-muted-foreground" :title="t('cryptoTrack.positions.approximateMark')">≈</span>
    <span >{{ formatPct(percent, { decimals: 1 }) }}</span>
  </span>
</template>
