<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { ArrowRight, TrendingDown, TrendingUp } from 'lucide-vue-next'

import { SIGN_TONE_CLASS, formatPct, signTone } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Sparkline } from '@/shared/ui/sparkline'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import type { MetricTileProps } from './metric-tile.types'

/**
 * The dashboard's figure (plan H1): the value, its change against a base,
 * a trend and a conclusion. The value and the money change come as slots —
 * money is drawn by `<Money>` from a higher layer.
 */
const props = withDefaults(defineProps<MetricTileProps>(), {
  deltaPct: null,
  goodWhen: 'up',
  baseLabel: undefined,
  series: () => [],
  insight: undefined,
  action: undefined,
  size: undefined,
})

const cellSize = useWidgetSize()
const level = computed(() => props.size ?? cellSize.value)

const direction = computed(() => signTone(props.deltaPct))
/** Green when the change goes the good way; a flat or unknown change stays neutral. */
const toneClass = computed(() => {
  if (direction.value === 'neutral') return 'text-muted-foreground'
  const good = (direction.value === 'positive') === (props.goodWhen === 'up')
  return SIGN_TONE_CLASS[good ? 'positive' : 'negative']
})
</script>

<template>
  <div class="flex min-w-0 flex-col gap-1">
    <p class="truncate text-xs text-muted-foreground">{{ props.label }}</p>
    <component
      :is="props.action ? RouterLink : 'p'"
      :to="props.action?.to"
      class="figure truncate"
      :class="props.action && 'rounded-sm hover:underline focus-visible:outline-2 focus-visible:outline-ring'"
    >
      <slot />
    </component>
    <!-- The change row is always there, so a value without a base does not shift what follows. -->
    <p class="flex min-w-0 items-center gap-1 text-xs" :class="toneClass">
      <template v-if="props.deltaPct !== null">
        <TrendingUp v-if="direction === 'positive'" class="size-3.5 shrink-0" aria-hidden="true" />
        <TrendingDown v-else-if="direction === 'negative'" class="size-3.5 shrink-0" aria-hidden="true" />
        <span class="truncate tabular-nums">
          <slot name="delta" />
          {{ formatPct(props.deltaPct, { signed: true, decimals: 1 }) }}
        </span>
      </template>
      <span v-if="props.baseLabel" class="truncate text-muted-foreground">{{ props.baseLabel }}</span>
    </p>
    <div v-if="level !== 's' && props.series.length > 1" :class="direction === 'neutral' ? 'text-primary' : toneClass">
      <Sparkline :values="props.series" />
    </div>
    <p v-if="level !== 's' && props.insight" class="line-clamp-2 text-xs text-muted-foreground">{{ props.insight }}</p>
    <slot name="detail" :level="level" />
    <Button v-if="level !== 's' && props.action" variant="link" size="sm" class="h-auto self-start p-0" as-child>
      <RouterLink :to="props.action.to">
        {{ props.action.label }}
        <ArrowRight class="size-3.5" />
      </RouterLink>
    </Button>
  </div>
</template>
