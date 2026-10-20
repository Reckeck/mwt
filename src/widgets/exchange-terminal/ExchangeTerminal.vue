<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ArrowUpToLine, LineChart, Plus, Trash2, X } from 'lucide-vue-next'

import { PriceAlertList } from '@/widgets/price-alert-list'
import { TradingChart, type IndicatorKey } from '@/widgets/trading-chart'
import { PriceAlertDialog, type PriceAlertPreset } from '@/features/price-alert-edit'
import { PairInput } from '@/entities/asset'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { required, useFormValidation } from '@/shared/lib/form'
import { notify } from '@/shared/lib/notify'
import { baseOf, pairOf } from '@/shared/lib/pair'

import type { ExchangeTerminalProps } from './exchange-terminal.types'
import { useTerminalDrawings } from './model/use-terminal-drawings'

const props = defineProps<ExchangeTerminalProps>()
const { t } = useI18n()
const queryClient = useQueryClient()

const queryKey = computed(() => ['terminal', props.storeId])
const terminal = useQuery({
  queryKey,
  queryFn: () => call<TerminalState>('get_terminal_state', { storeId: props.storeId }),
  staleTime: Number.POSITIVE_INFINITY,
})

const fail = (err: unknown) => notify.error(err, 'terminal.saveFailed')

const patch = useMutation({
  mutationFn: (change: TerminalPatch) =>
    call<TerminalState>('update_terminal_state', { input: { store_id: props.storeId, patch: change } }),
  onSuccess: (state) => queryClient.setQueryData(queryKey.value, state),
  onError: fail,
  meta: { silent: 'the layout saves as you work on it; the chart shows the change' },
})

const watchlist = computed(() => terminal.data.value?.watchlist ?? [])
const active = ref('')
watch(
  () => terminal.data.value,
  (state) => {
    if (state && !active.value) active.value = state.active_symbol ?? state.watchlist[0] ?? 'BTCUSDT'
  },
  { immediate: true },
)

const drawings = useTerminalDrawings({
  storeId: () => props.storeId,
  queryKey,
  state: () => terminal.data.value,
  active,
  fail,
})
const { chartNonce } = drawings

function select(pair: string) {
  active.value = pair
  patch.mutate({ watchlist: null, active_symbol: pair, timeframe: null, indicators: null, layout: null })
}

function moveTop(pair: string) {
  const next = [pair, ...watchlist.value.filter((entry) => entry !== pair)]
  patch.mutate({ watchlist: next, active_symbol: null, timeframe: null, indicators: null, layout: null })
}

function watchActions(pair: string): UiAction[] {
  return [
    { id: 'open', label: t('terminal.open'), icon: LineChart, run: () => select(pair) },
    { id: 'top', label: t('terminal.moveTop'), icon: ArrowUpToLine, disabled: watchlist.value[0] === pair, run: () => moveTop(pair) },
    {
      id: 'clear',
      label: t('terminal.clearDrawings'),
      icon: Trash2,
      separated: true,
      disabled: !terminal.data.value?.drawings[pair],
      run: () => void drawings.clear(pair),
    },
    { id: 'remove', label: t('terminal.remove', { symbol: pair }), icon: X, destructive: true, run: () => removeFromList(pair) },
  ]
}

const pairDraft = ref('')
const adder = useFormValidation(() => ({
  pair: required(pairDraft.value) ?? (pairOf(pairDraft.value) ? null : t('validation.pair')),
}))
function add() {
  const pair = pairOf(pairDraft.value)
  if (!pair) return
  pairDraft.value = ''
  adder.reset()
  if (!watchlist.value.includes(pair)) {
    patch.mutate({ watchlist: [...watchlist.value, pair], active_symbol: pair, timeframe: null, indicators: null, layout: null })
  }
  active.value = pair
}

function removeFromList(pair: string) {
  const before = [...watchlist.value]
  const next = watchlist.value.filter((entry) => entry !== pair)
  patch.mutate({ watchlist: next, active_symbol: null, timeframe: null, indicators: null, layout: null })
  if (active.value === pair) active.value = next[0] ?? 'BTCUSDT'
  notify.success(t('terminal.removedFromList', { symbol: baseOf(pair) }), {
    action: {
      label: t('actions.undo'),
      onClick: () => patch.mutate({ watchlist: before, active_symbol: null, timeframe: null, indicators: null, layout: null }),
    },
  })
}

const indicatorKeys = computed<IndicatorKey[]>(() =>
  (terminal.data.value?.indicators ?? [])
    .map((entry) => entry.kind as IndicatorKey)
    .filter((kind) => ['sma', 'ema', 'rsi'].includes(kind)),
)

/* --- alerts set from the chart belong to this exchange --- */
const alertOpen = ref(false)
const alertPreset = ref<PriceAlertPreset | null>(null)
function onAlert(price: string, condition: AlertCondition) {
  alertPreset.value = { symbol: baseOf(active.value || 'BTCUSDT'), price, condition }
  alertOpen.value = true
}

function onTimeframe(label: string) {
  patch.mutate({ watchlist: null, active_symbol: null, timeframe: label, indicators: null, layout: null })
}

function onIndicators(keys: IndicatorKey[]) {
  const periods: Record<IndicatorKey, number> = { sma: 20, ema: 50, rsi: 14 }
  patch.mutate({
    watchlist: null,
    active_symbol: null,
    timeframe: null,
    indicators: keys.map((kind) => ({ kind, period: periods[kind] })),
    layout: null,
  })
}
</script>

<template>
  <SectionState
    :loading="terminal.isPending.value"
    :error="terminal.error.value"
    skeleton="chart"
    @retry="terminal.refetch()"
  >
  <div class="grid min-h-[32rem] gap-3 @container lg:grid-cols-[14rem_1fr]" data-tour="exchange-terminal">
    <aside class="flex flex-col gap-2 rounded-lg border border-border p-2">
      <form class="grid gap-1" @submit.prevent="adder.submit(add, $event)">
        <div class="flex items-center gap-1">
          <div class="min-w-0 flex-1" :data-invalid="adder.invalidAttr('pair')">
            <PairInput v-model="pairDraft" :quotes="['USDT']" :placeholder="t('trading.symbol')" />
          </div>
          <Button type="submit" variant="ghost" size="icon-sm"  :title="t('terminal.add')" :aria-label="t('terminal.add')">
            <Plus class="size-4" />
          </Button>
        </div>
        <p v-if="adder.firstMessage()" class="text-xs text-destructive" role="alert">{{ adder.firstMessage() }}</p>
      </form>
      <ul class="flex flex-col gap-0.5 overflow-y-auto" :aria-label="t('trading.watchlist')">
        <ContextMenu v-for="pair in watchlist" :key="pair" :actions="watchActions(pair)">
        <li class="group flex items-center">
          <Button variant="ghost" size="block" class="block"
            :class="
              cn(
                'min-w-0 flex-1 truncate rounded-md px-2 py-1.5 text-left text-sm font-medium transition-colors hover:bg-accent',
                active === pair && 'bg-accent text-accent-foreground',
              )
            "
            @click="select(pair)"
          >
            {{ baseOf(pair) }}<span class="text-xs text-muted-foreground">/USDT</span>
          </Button>
          <Button
            variant="ghost"
            size="icon-xs"
            class="invisible text-muted-foreground hover:text-destructive group-hover:visible focus-visible:visible pointer-coarse:visible"
            :aria-label="t('terminal.remove', { symbol: pair })"
            @click="removeFromList(pair)"
          >
            <X class="size-3.5" />
          </Button>
          <span class="pointer-fine:hidden"><ActionMenu :actions="watchActions(pair)" :label="t('actions.more')" /></span>
        </li>
        </ContextMenu>
      </ul>
      <PriceAlertList :store-id="props.storeId" class="mt-auto border-t border-border pt-2" />
    </aside>

    <TradingChart
      :key="`${props.storeId}:${chartNonce}`"
      :symbol="baseOf(active || 'BTCUSDT')"
      :saved-drawings="drawings.saved.value"
      :timeframe="terminal.data.value?.timeframe ?? '1h'"
      :indicators="indicatorKeys.length ? indicatorKeys : ['sma']"
      class="min-h-[30rem]"
      can-alert
      @alert="onAlert"
      @drawings="drawings.onDrawings"
      @timeframe="onTimeframe"
      @indicators="onIndicators"
    />
    <PriceAlertDialog v-model:open="alertOpen" :store-id="props.storeId" :preset="alertPreset" />
  </div>
  </SectionState>
</template>
