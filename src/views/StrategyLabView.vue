<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Play, Plus, Save, Trash2, Eye, EyeOff } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Select from '@/components/ui/select.vue'
import { Chart, type ChartOption } from '@/lib/echarts'
import { call, parseError } from '@/lib/ipc'
import { useTheme } from '@/theme'

const { t } = useI18n()
const { resolved } = useTheme()

const strategies = useQuery({
  queryKey: ['strategies'],
  queryFn: () => call<Strategy[]>('list_strategies'),
})

const runs = useQuery({
  queryKey: ['runs'],
  queryFn: () => call<SimRun[]>('list_runs'),
})

function blankRule(): Rule {
  return { indicator: 'sma20', field: 'close', op: 'cross_up', value: '0' }
}

type Group = 'entry' | 'exit'

const editing = reactive({
  name: 'Golden Bell',
  symbol: 'BTC',
  source: 'synthetic' as DataSource,
  initial_capital: '10000',
  position_size_pct: '0.5',
  entry: [blankRule()] as Rule[],
  exit: [blankRule()] as Rule[],
  stop_loss_pct: '10',
})

const candles = ref(500)
const granularity_min = ref(60)

const indicators = ['close', 'sma20', 'sma50', 'ema12', 'ema26', 'rsi14'] as const
const ops = [
  { value: 'gt', label: '>' },
  { value: 'lt', label: '<' },
  { value: 'cross_up', label: 'cross ↑' },
  { value: 'cross_down', label: 'cross ↓' },
] as const

const axisColor = computed(() => (resolved.value === 'dark' ? 'rgba(255,255,255,0.45)' : 'rgba(0,0,0,0.45)'))

// Selected run for detail view
const selectedRunId = ref<string>('')
watch(runs.data, () => {
  if (!selectedRunId.value && runs.data.value?.length) {
    selectedRunId.value = runs.data.value[0].id
  }
})
const selectedRun = computed(() => runs.data.value?.find((run) => run.id === selectedRunId.value) ?? null)

// Candles query for selected run
const { data: runCandles, isLoading: candlesLoading, refetch: refetchCandles } = useQuery({
  queryKey: ['run-candles', selectedRunId],
  queryFn: () => call<GetRunCandlesOutput>('get_run_candles', { input: { run_id: selectedRunId.value ?? '' } }),
  enabled: !!selectedRunId.value,
})

// GBM paths query for fan chart
const showFan = ref(false)
const { data: gbmData, refetch: refetchGbm } = useQuery({
  queryKey: ['gbm', selectedRunId, showFan],
  queryFn: () =>
    call<GbmPathsOutput>('get_gbm_paths', {
      input: { run_id: selectedRunId.value ?? '', paths: 20, horizon: 100 },
    }),
  enabled: showFan.value && !!selectedRunId.value,
})

function addRule(group: Group) {
  editing[group].push(blankRule())
}

function removeRule(group: Group, index: number) {
  editing[group].splice(index, 1)
}

function buildStrategy(): Strategy {
  return {
    id: crypto.randomUUID(),
    name: editing.name,
    symbol: editing.symbol.toUpperCase(),
    chain: null,
    source: editing.source,
    initial_capital: editing.initial_capital,
    asset: {
      symbol: editing.symbol.toUpperCase(),
      name: editing.symbol.toUpperCase(),
      coingecko_id: null,
      chain: null,
      contract: null,
      quote: editing.symbol.toUpperCase(),
    },
    entry_rules: editing.entry,
    exit_rules: editing.exit,
    stop_loss_pct: editing.stop_loss_pct,
    take_profit_pct: null,
    position_size_pct: editing.position_size_pct,
    created_at: new Date().toISOString(),
  }
}

/** `quiet` is for the implicit save inside `runStrategy`. */
async function saveStrategy(quiet = false) {
  try {
    await call('save_strategy', { input: { strategy: buildStrategy() } })
    await strategies.refetch()
    if (!quiet) toast.success(t('sim.saved'), { description: editing.name })
  } catch (err) {
    toast.error(t('sim.saveFailed'), { description: parseError(err).message })
    throw err
  }
}

async function runStrategy() {
  try {
    let selected = (strategies.data.value ?? []).find((strategy) => strategy.name === editing.name)
    if (!selected) {
      await saveStrategy(true)
      selected = (await call<Strategy[]>('list_strategies')).find((strategy) => strategy.name === editing.name)
    }
    if (!selected) return
    const run = await call<SimRun>('run_strategy', {
      input: { strategy_id: selected.id, candles: candles.value, granularity_min: granularity_min.value },
    })
    toast.success(t('sim.ran'), { description: run.id })
    await runs.refetch()
  } catch (err) {
    toast.error(t('sim.runFailed'), { description: parseError(err).message })
  }
}

async function onRunChange(runId: string) {
  selectedRunId.value = runId
  await refetchCandles()
  if (showFan.value) await refetchGbm()
}

// Equity curve chart
const equityOption = computed<ChartOption>(() => {
  const curve = selectedRun.value?.equity_curve ?? []
  return {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis' },
    grid: { left: 48, right: 16, top: 16, bottom: 28, containLabel: true },
    xAxis: { type: 'time', axisLabel: { color: axisColor.value }, axisLine: { lineStyle: { color: axisColor.value } } },
    yAxis: {
      type: 'value',
      scale: true,
      axisLabel: { color: axisColor.value },
      splitLine: { lineStyle: { color: axisColor.value, opacity: 0.12 } },
    },
    series: [
      {
        type: 'line',
        showSymbol: false,
        smooth: true,
        name: 'equity',
        data: curve.map((point) => [point.ts, Number(point.equity)]),
      },
    ],
  }
})

// Candlestick chart with trades, signals, and GBM fan
const candleOption = computed<ChartOption>(() => {
  const candles = runCandles.value?.candles ?? []
  if (!candles.length) return {}

  const run = selectedRun.value
  const trades = run?.trades ?? []
  const signals = run?.signals ?? []

  const ohlc = candles.map((candle: CandlePoint) => [
    candle.ts,
    Number(candle.open),
    Number(candle.close),
    Number(candle.low),
    Number(candle.high),
  ])

  // Trade markers: buy/sell
  const buyMarkers = trades
    .filter((trade) => trade.side === 'buy')
    .map((trade) => ({ ts: trade.ts, price: Number(trade.price), type: 'buy' as const }))
  const sellMarkers = trades
    .filter((trade) => trade.side === 'sell')
    .map((trade) => ({ ts: trade.ts, price: Number(trade.price), type: 'sell' as const }))

  // Signal markers: entry/exit with detail
  const entrySignals = signals
    .filter((signal) => signal.side === 'entry')
    .map((signal) => ({ ts: signal.ts, price: Number(signal.price), detail: signal.detail, reason: signal.reason }))
  const exitSignals = signals
    .filter((signal) => signal.side === 'exit')
    .map((signal) => ({ ts: signal.ts, price: Number(signal.price), detail: signal.detail, reason: signal.reason }))

  // GBM fan paths
  const fanPaths = gbmData.value?.paths ?? []

  const series: ChartOption['series'] = [
    {
      type: 'candlestick',
      name: 'OHLC',
      data: ohlc,
      itemStyle: {
        color: '#22c55e',
        color0: '#ef4444',
        borderColor: '#22c55e',
        borderColor0: '#ef4444',
      },
    },
  ]

  // Volume as separate y-axis or bar - skip for simplicity, use candles only
  // Add trade markers as scatter
  if (buyMarkers.length) {
    series.push({
      type: 'scatter',
      name: t('sim.buy'),
      symbol: 'triangle',
      symbolSize: 10,
      symbolRotate: 90,
      itemStyle: { color: '#22c55e' },
      data: buyMarkers.map((marker) => ({ value: [marker.ts, marker.price], itemStyle: { color: '#22c55e' } })),
      tooltip: { formatter: (point: unknown) => `${t('sim.buy')}<br/>${(point as { value?: (string | number)[] })?.value?.[0]}<br/>${(point as { value?: (string | number)[] })?.value?.[1]}` },
    })
  }
  if (sellMarkers.length) {
    series.push({
      type: 'scatter',
      name: t('sim.sell'),
      symbol: 'triangle',
      symbolSize: 10,
      symbolRotate: -90,
      itemStyle: { color: '#ef4444' },
      data: sellMarkers.map((marker) => ({ value: [marker.ts, marker.price], itemStyle: { color: '#ef4444' } })),
      tooltip: { formatter: (point: unknown) => `${t('sim.sell')}<br/>${(point as { value?: (string | number)[] })?.value?.[0]}<br/>${(point as { value?: (string | number)[] })?.value?.[1]}` },
    })
  }
  // Signal markers
  if (entrySignals.length) {
    series.push({
      type: 'scatter',
      name: t('sim.signal.entry'),
      symbol: 'pin',
      symbolSize: 8,
      symbolOffset: [0, '-50%'],
      itemStyle: { color: '#3b82f6' },
      data: entrySignals.map((signal) => ({ value: [signal.ts, signal.price], detail: signal.detail, reason: signal.reason })),
      tooltip: { formatter: (point: unknown) => `${t('sim.signal.entry')}<br/>${(point as { value?: (string | number)[] })?.value?.[0]}<br/>${(point as { data?: { reason: string; detail: string } })?.data?.reason}: ${(point as { data?: { reason: string; detail: string } })?.data?.detail}` },
    })
  }
  if (exitSignals.length) {
    series.push({
      type: 'scatter',
      name: t('sim.signal.exit'),
      symbol: 'pin',
      symbolSize: 8,
      symbolOffset: [0, '-50%'],
      itemStyle: { color: '#f97316' },
      data: exitSignals.map((signal) => ({ value: [signal.ts, signal.price], detail: signal.detail, reason: signal.reason })),
      tooltip: { formatter: (point: unknown) => `${t('sim.signal.exit')}<br/>${(point as { value?: (string | number)[] })?.value?.[0]}<br/>${(point as { data?: { reason: string; detail: string } })?.data?.reason}: ${(point as { data?: { reason: string; detail: string } })?.data?.detail}` },
    })
  }
  // GBM fan paths as low-opacity lines
  if (fanPaths.length) {
    fanPaths.forEach((path: GbmPathPoint[], idx: number) => {
      series.push({
        type: 'line',
        name: `${t('sim.fanPath')} ${idx + 1}`,
        showSymbol: false,
        lineStyle: { width: 1, opacity: 0.15, color: '#888' },
        data: path.map((point: GbmPathPoint) => [point.ts, Number(point.price)]),
        tooltip: { show: false },
      })
    })
  }

  return {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis', axisPointer: { type: 'cross' } },
    grid: { left: 48, right: 16, top: 16, bottom: 28, containLabel: true },
    xAxis: { type: 'time', axisLabel: { color: axisColor.value }, axisLine: { lineStyle: { color: axisColor.value } } },
    yAxis: {
      type: 'value',
      scale: true,
      axisLabel: { color: axisColor.value },
      splitLine: { lineStyle: { color: axisColor.value, opacity: 0.12 } },
    },
    dataZoom: [{ type: 'inside' }, { type: 'slider', bottom: 0, height: 16 }],
    series,
    legend: { top: 0, textStyle: { color: axisColor.value } },
  }
})

const formatSignalSide = (side: string) => t(`sim.signal.side.${side}`)
const formatSignalReason = (reason: string) => t(`sim.signal.reason.${reason}`)
</script>

<template>
  <div class="grid gap-6 @xl:grid-cols-2">
    <div class="space-y-4">
      <Card>
        <template #header>
          <p class="font-medium">{{ t('sim.title') }}</p>
        </template>
        <div class="grid gap-4">
          <div class="grid grid-cols-2 gap-4">
            <div class="grid gap-1.5">
              <Label>{{ t('sim.name') }}</Label>
              <Input v-model="editing.name" />
            </div>
            <div class="grid gap-1.5">
              <Label>{{ t('sim.symbol') }}</Label>
              <Input v-model="editing.symbol" />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-4">
            <div class="grid gap-1.5">
              <Label>{{ t('sim.source') }}</Label>
              <select v-model="editing.source" class="h-9 rounded-md border border-input bg-transparent px-3 text-sm">
                <option value="real">{{ t('sim.source.real') }}</option>
                <option value="synthetic">{{ t('sim.source.synthetic') }}</option>
              </select>
            </div>
            <div class="grid gap-1.5">
              <Label>{{ t('sim.initialCapital') }}</Label>
              <Input v-model="editing.initial_capital" />
            </div>
          </div>

          <div class="grid grid-cols-2 gap-4">
            <div class="grid gap-1.5">
              <Label>{{ t('sim.positionSize') }}</Label>
              <Input v-model="editing.position_size_pct" />
            </div>
            <div class="grid gap-1.5">
              <Label>{{ t('sim.candles') }}</Label>
              <Input v-model.number="candles" type="number" />
            </div>
          </div>

          <div class="grid gap-1.5">
            <Label>{{ t('sim.granularity') }}</Label>
            <Input v-model.number="granularity_min" type="number" min="1" />
          </div>

          <div v-for="group in (['entry', 'exit'] as Group[])" :key="group" class="space-y-2">
            <div class="flex items-center justify-between">
              <Label>{{ group === 'entry' ? t('sim.entryRules') : t('sim.exitRules') }}</Label>
              <Button variant="secondary" size="sm" @click="addRule(group)">
                <Plus class="size-3" />
              </Button>
            </div>
            <div v-for="(r, i) in editing[group]" :key="i" class="flex items-center gap-2">
              <select v-model="r.indicator" class="h-8 grow rounded-md border border-input bg-transparent px-2 text-sm">
                <option v-for="ind in indicators" :key="ind" :value="ind">{{ ind }}</option>
              </select>
              <select v-model="r.op" class="h-8 rounded-md border border-input bg-transparent px-2 text-sm">
                <option v-for="op in ops" :key="op.value" :value="op.value">{{ op.label }}</option>
              </select>
              <Input v-model="r.value" class="h-8 w-24" />
              <Button variant="ghost" size="icon" @click="removeRule(group, i)">
                <Trash2 class="size-4 text-destructive" />
              </Button>
            </div>
          </div>

          <div class="flex gap-2">
            <Button @click="saveStrategy">
              <Save class="size-4" />
              {{ t('sim.save') }}
            </Button>
            <Button variant="outline" @click="runStrategy">
              <Play class="size-4" />
              {{ t('sim.run') }}
            </Button>
          </div>
        </div>
      </Card>
    </div>

    <div class="space-y-4">
      <Card>
        <template #header>
          <p class="font-medium">{{ t('sim.runs') }} · {{ runs.data.value?.length ?? 0 }}</p>
        </template>

        <div v-if="(runs.data.value ?? []).length">
          <div class="mb-4">
            <Label class="block text-sm mb-1">{{ t('sim.selectRun') }}</Label>
            <Select v-model="selectedRunId" @update:model-value="onRunChange($event)">
              <option v-for="run in runs.data.value" :key="run.id" :value="run.id">
                {{ run.strategy_id.slice(0, 8) }} · {{ run.started_at }} · {{ run.total_trades }} trades
              </option>
            </Select>
          </div>

          <div class="space-y-4">
            <Card>
              <template #header>
                <div class="flex items-center justify-between">
                  <p class="font-medium">{{ t('sim.candleChart') }}</p>
                  <div class="flex items-center gap-2">
                    <Button variant="outline" size="sm" @click="showFan = !showFan">
                      <Eye v-if="!showFan" class="size-3" />
                      <EyeOff v-else class="size-3" />
                      {{ showFan ? t('sim.hideFan') : t('sim.showFan') }}
                    </Button>
                  </div>
                </div>
              </template>
              <div v-if="candlesLoading" class="py-10 text-center text-sm text-muted-foreground">{{ t('sim.loading') }}</div>
              <Chart v-else :option="candleOption" autoresize class="h-96 w-full" />
            </Card>

            <Card>
              <template #header>
                <p class="font-medium">{{ t('sim.equityCurve') }}</p>
              </template>
              <Chart :option="equityOption" autoresize class="h-72 w-full" />
            </Card>

            <div class="mt-4 grid grid-cols-2 gap-4 text-sm @sm:grid-cols-4">
              <div class="rounded-md bg-muted p-4">
                <p class="text-xs text-muted-foreground">{{ t('sim.finalEquity') }}</p>
                <p class="font-semibold tabular-nums">{{ selectedRun?.final_equity }}</p>
              </div>
              <div class="rounded-md bg-muted p-4">
                <p class="text-xs text-muted-foreground">{{ t('sim.return') }}</p>
                <p class="font-semibold tabular-nums">{{ selectedRun?.total_return_pct }}%</p>
              </div>
              <div class="rounded-md bg-muted p-4">
                <p class="text-xs text-muted-foreground">{{ t('sim.maxDd') }}</p>
                <p class="font-semibold tabular-nums">{{ selectedRun?.max_drawdown_pct }}%</p>
              </div>
              <div class="rounded-md bg-muted p-4">
                <p class="text-xs text-muted-foreground">{{ t('sim.trades') }}</p>
                <p class="font-semibold tabular-nums">{{ selectedRun?.total_trades }}</p>
              </div>
            </div>

            <Card v-if="selectedRun?.signals?.length">
              <template #header>
                <p class="font-medium">{{ t('sim.signals') }} ({{ selectedRun.signals.length }})</p>
              </template>
              <div class="overflow-x-auto">
                <table class="w-full text-sm">
                  <thead>
                    <tr class="border-b border-border">
                      <th class="text-left py-2 px-3">{{ t('sim.signal.ts') }}</th>
                      <th class="text-center py-2 px-3">{{ t('sim.signal.side') }}</th>
                      <th class="text-left py-2 px-3">{{ t('sim.signal.reason') }}</th>
                      <th class="text-left py-2 px-3">{{ t('sim.signal.detail') }}</th>
                      <th class="text-right py-2 px-3">{{ t('sim.signal.price') }}</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-border">
                    <tr v-for="signal in selectedRun.signals" :key="signal.ts" class="hover:bg-muted/50">
                      <td class="py-2 px-3 font-mono">{{ signal.ts }}</td>
                      <td class="py-2 px-3 text-center capitalize">{{ formatSignalSide(signal.side) }}</td>
                      <td class="py-2 px-3 capitalize">{{ formatSignalReason(signal.reason) }}</td>
                      <td class="py-2 px-3 max-w-xs truncate">{{ signal.detail }}</td>
                      <td class="py-2 px-3 text-right font-mono tabular-nums">{{ Number(signal.price).toFixed(4) }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </Card>
          </div>
        </div>
        <p v-else class="py-10 text-center text-sm text-muted-foreground">{{ t('sim.empty') }}</p>
      </Card>

      <Card v-if="(strategies.data.value ?? []).length">
        <template #header>
          <p class="font-medium">{{ t('nav.strategyLab') }}</p>
        </template>
        <div class="divide-y divide-border">
          <div v-for="strategy in strategies.data.value ?? []" :key="strategy.id" class="flex items-center justify-between py-3">
            <div>
              <p class="font-medium">{{ strategy.name }}</p>
              <p class="text-xs text-muted-foreground">{{ strategy.symbol }} · {{ strategy.source }}</p>
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>