<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Play, Plus, Save, Trash2 } from 'lucide-vue-next'

import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { Chart, type ChartOption } from '@/lib/echarts'
import { call } from '@/lib/ipc'
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

const equityOption = computed<ChartOption>(() => ({
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
      data: (runs.data.value?.[0]?.equity_curve ?? []).map((p) => [p.ts, Number(p.equity)]),
    },
  ],
}))

function addRule(group: Group) {
  editing[group].push(blankRule())
}

function removeRule(group: Group, i: number) {
  editing[group].splice(i, 1)
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

async function saveStrategy() {
  await call('save_strategy', { input: { strategy: buildStrategy() } })
  await strategies.refetch()
}

async function runStrategy() {
  let selected = (strategies.data.value ?? []).find((s) => s.name === editing.name)
  if (!selected) {
    await saveStrategy()
    selected = (await call<Strategy[]>('list_strategies')).find((s) => s.name === editing.name)
  }
  if (!selected) return
  await call<SimRun>('run_strategy', {
    input: { strategy_id: selected.id, candles: candles.value, granularity_min: granularity_min.value },
  })
  await runs.refetch()
}
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
                <option v-for="o in ops" :key="o.value" :value="o.value">{{ o.label }}</option>
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
          <Chart :option="equityOption" autoresize class="h-72 w-full" />

          <div class="mt-4 grid grid-cols-2 gap-2 text-sm @sm:grid-cols-4">
            <div class="rounded-md bg-muted p-3">
              <p class="text-xs text-muted-foreground">{{ t('sim.finalEquity') }}</p>
              <p class="font-semibold tabular-nums">{{ runs.data.value?.[0]?.final_equity }}</p>
            </div>
            <div class="rounded-md bg-muted p-3">
              <p class="text-xs text-muted-foreground">{{ t('sim.return') }}</p>
              <p class="font-semibold tabular-nums">{{ runs.data.value?.[0]?.total_return_pct }}%</p>
            </div>
            <div class="rounded-md bg-muted p-3">
              <p class="text-xs text-muted-foreground">{{ t('sim.maxDd') }}</p>
              <p class="font-semibold tabular-nums">{{ runs.data.value?.[0]?.max_drawdown_pct }}%</p>
            </div>
            <div class="rounded-md bg-muted p-3">
              <p class="text-xs text-muted-foreground">{{ t('sim.trades') }}</p>
              <p class="font-semibold tabular-nums">{{ runs.data.value?.[0]?.total_trades }}</p>
            </div>
          </div>
        </div>
        <p v-else class="py-10 text-center text-sm text-muted-foreground">{{ t('sim.empty') }}</p>
      </Card>

      <Card v-if="(strategies.data.value ?? []).length">
        <template #header>
          <p class="font-medium">{{ t('nav.strategyLab') }}</p>
        </template>
        <div class="divide-y divide-border">
          <div v-for="s in strategies.data.value ?? []" :key="s.id" class="flex items-center justify-between py-2">
            <div>
              <p class="font-medium">{{ s.name }}</p>
              <p class="text-xs text-muted-foreground">{{ s.symbol }} · {{ s.source }}</p>
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</template>