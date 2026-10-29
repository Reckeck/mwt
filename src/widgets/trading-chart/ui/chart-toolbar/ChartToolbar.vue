<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import type { DrawingKind } from 'lightweight-charts-drawing'
import { BellPlus, Magnet, RefreshCw, Trash2, Undo2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { Toggle } from '@/shared/ui/toggle'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { cn } from '@/shared/lib/utils'

import { tfLabel } from '../../lib/timeframe'
import type { StreamStatus } from '../../model/use-binance-stream'
import { CHART_TIMEFRAMES, DRAWING_TOOLS, INDICATORS, type IndicatorKey } from '../../trading-chart.constants'

/** Timeframe, indicators, drawing tools, and whether the live stream is up. */
const props = defineProps<{
  tf: number
  indicators: IndicatorKey[]
  activeTool: DrawingKind | null
  canUndo: boolean
  hasDrawings: boolean
  refreshing: boolean
  stream: StreamStatus
  /** `Price alert` is offered (a price alert can be set for this symbol). */
  canAlert?: boolean
}>()
const magnet = defineModel<boolean>('magnet', { required: true })
const emit = defineEmits<{
  timeframe: [minutes: number]
  indicators: [keys: IndicatorKey[]]
  tool: [value: unknown]
  undo: []
  clear: []
  alert: []
}>()

const { t } = useI18n()

/** The timeframe always has a value: pressing the active one again is ignored. */
function pickTimeframe(value: unknown) {
  if (typeof value === 'string' && value) emit('timeframe', Number(value))
}
function pickIndicators(value: unknown) {
  emit('indicators', Array.isArray(value) ? (value as IndicatorKey[]) : [])
}

const STREAM_DOT: Record<StreamStatus, string> = { live: 'bg-success', connecting: 'bg-warning', down: 'bg-muted-foreground' }
</script>

<template>
  <div class="flex flex-wrap items-center gap-2">
    <ToggleGroup
      type="single"
      variant="outline"
      size="sm"
      :model-value="String(props.tf)"
      :aria-label="t('trading.timeframe')"
      @update:model-value="pickTimeframe"
    >
      <ToggleGroupItem v-for="frame in CHART_TIMEFRAMES" :key="frame" :value="String(frame)" class="tabular-nums">
        {{ tfLabel(frame) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <ToggleGroup
      type="multiple"
      variant="outline"
      size="sm"
      :model-value="props.indicators"
      :aria-label="t('trading.indicators')"
      @update:model-value="pickIndicators"
    >
      <ToggleGroupItem v-for="ind in INDICATORS" :key="ind.key" :value="ind.key">{{ ind.label }}</ToggleGroupItem>
    </ToggleGroup>

    <div class="flex items-center gap-1">
      <ToggleGroup
        type="single"
        size="sm"
        :model-value="props.activeTool ?? ''"
        :aria-label="t('trading.drawing')"
        @update:model-value="(value) => emit('tool', value)"
      >
        <Tooltip v-for="tool in DRAWING_TOOLS" :key="tool.kind">
          <TooltipTrigger as-child>
            <ToggleGroupItem :value="tool.kind" :aria-label="t(tool.key)">
              <component :is="tool.icon" />
            </ToggleGroupItem>
          </TooltipTrigger>
          <TooltipContent>{{ t(tool.key) }}</TooltipContent>
        </Tooltip>
      </ToggleGroup>

      <Separator orientation="vertical" class="mx-1 data-[orientation=vertical]:h-5" />

      <Tooltip>
        <TooltipTrigger as-child>
          <Toggle v-model="magnet" size="sm" :aria-label="t('trading.magnet')"><Magnet /></Toggle>
        </TooltipTrigger>
        <TooltipContent>{{ t('trading.magnet') }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button variant="ghost" size="icon-sm"  :aria-label="t('trading.undo')" :disabled="!props.canUndo" @click="emit('undo')">
            <Undo2 class="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{{ t('trading.undo') }}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button
            variant="ghost"
            size="icon-sm"
            :aria-label="t('trading.clearDrawings')"
            :disabled="!props.hasDrawings"
            @click="emit('clear')"
          >
            <Trash2 class="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{{ t('trading.clearDrawings') }}</TooltipContent>
      </Tooltip>
      <!-- The right-click menu sets an alert at the price under the cursor; this does it from the
           last price, for a mouse-only user and a finger alike (rule 16). -->
      <Tooltip v-if="props.canAlert">
        <TooltipTrigger as-child>
          <Button variant="ghost" size="icon-sm" :aria-label="t('terminal.alertHere')" @click="emit('alert')">
            <BellPlus class="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{{ t('terminal.alertHere') }}</TooltipContent>
      </Tooltip>
    </div>

    <div class="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
      <RefreshCw v-if="props.refreshing" class="size-3.5 animate-spin" />
      <span :class="cn('size-2 rounded-full', STREAM_DOT[props.stream])" />
      {{ props.stream === 'live' ? t('trading.live') : props.stream === 'connecting' ? t('trading.connecting') : t('trading.down') }}
    </div>
  </div>
</template>
