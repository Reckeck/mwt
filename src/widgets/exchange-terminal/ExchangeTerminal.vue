<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useDebounceFn } from '@vueuse/core'
import { ArrowUpToLine, LineChart, Plus, Search, Trash2, X } from 'lucide-vue-next'

import { PriceAlertList } from '@/widgets/price-alert-list'
import { TradingChart, type IndicatorKey } from '@/widgets/trading-chart'
import { PriceAlertDialog, type PriceAlertPreset } from '@/features/price-alert-edit'
import { Button } from '@/shared/ui/button'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { Input } from '@/shared/ui/input'
import { call, parseError } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'
import { required, useFormValidation } from '@/shared/lib/form'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { notify } from '@/shared/lib/notify'

import type { ExchangeTerminalProps } from './exchange-terminal.types'

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

/** Watchlist entries are pairs against USDT; the chart takes the base ticker. */
const baseOf = (pair: string) => pair.toUpperCase().replace(/USDT$/, '')
const pairOf = (raw: string) => {
  const base = raw.trim().toUpperCase().replace(/[/\-_ ]/g, '').replace(/USDT?$/, '')
  return base ? `${base}USDT` : ''
}

const watchlist = computed(() => terminal.data.value?.watchlist ?? [])
const active = ref('')
watch(
  () => terminal.data.value,
  (state) => {
    if (state && !active.value) active.value = state.active_symbol ?? state.watchlist[0] ?? 'BTCUSDT'
  },
  { immediate: true },
)

function select(pair: string) {
  active.value = pair
  patch.mutate({ watchlist: null, active_symbol: pair, timeframe: null, indicators: null, layout: null })
}

function moveTop(pair: string) {
  const next = [pair, ...watchlist.value.filter((entry) => entry !== pair)]
  patch.mutate({ watchlist: next, active_symbol: null, timeframe: null, indicators: null, layout: null })
}

/** Bumped to remount the chart when its drawings change from outside it. */
const chartNonce = ref(0)

/** Write one pair's drawings and show them; `json` of `'[]'` clears them. */
async function writeDrawings(pair: string, json: string) {
  await call('save_terminal_drawings', { input: { store_id: props.storeId, symbol: pair, json } })
  queryClient.setQueryData<TerminalState>(queryKey.value, (old) => {
    if (!old) return old
    const { [pair]: _previous, ...drawings } = old.drawings
    return { ...old, drawings: json === '[]' ? drawings : { ...drawings, [pair]: json } }
  })
  if (pair === active.value) chartNonce.value += 1
}

async function clearDrawings(pair: string) {
  pendingDrawings.delete(pair)
  const before = terminal.data.value?.drawings[pair]
  try {
    await writeDrawings(pair, '[]')
    notify.success(t('terminal.drawingsCleared', { symbol: baseOf(pair) }), {
      action: before
        ? { label: t('actions.undo'), onClick: () => void writeDrawings(pair, before).catch(fail) }
        : undefined,
    })
  } catch (err) {
    fail(err)
  }
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
      run: () => void clearDrawings(pair),
    },
    { id: 'remove', label: t('terminal.remove', { symbol: pair }), icon: X, destructive: true, run: () => removeFromList(pair) },
  ]
}

const draft = ref('')
const adder = useFormValidation(() => ({
  pair: required(draft.value) ?? (pairOf(draft.value) ? null : t('validation.pair')),
}))
function add() {
  const pair = pairOf(draft.value)
  if (!pair) return
  draft.value = ''
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

/* --- chart state goes to the vault, debounced: a drag fires many changes --- */
const pendingDrawings = new Map<string, string>()
const flushDrawings = useDebounceFn(async () => {
  const batch = [...pendingDrawings.entries()]
  pendingDrawings.clear()
  try {
    for (const [symbol, json] of batch) {
      await call('save_terminal_drawings', { input: { store_id: props.storeId, symbol, json } })
    }
    queryClient.setQueryData<TerminalState>(queryKey.value, (old) => {
      if (!old) return old
      const drawings = { ...old.drawings }
      for (const [symbol, json] of batch) {
        if (json.trim() === '[]' || !json.trim()) delete drawings[symbol]
        else drawings[symbol] = json
      }
      return { ...old, drawings }
    })
  } catch (err) {
    fail(err)
  }
}, 1000)

function onDrawings(symbol: string, json: string) {
  pendingDrawings.set(pairOf(symbol), json)
  void flushDrawings()
}

/** The chart keys drawings by its base ticker; the vault by pair. */
const savedDrawings = computed(() => {
  const out: Record<string, string> = {}
  for (const [pair, json] of Object.entries(terminal.data.value?.drawings ?? {})) {
    if (json) out[baseOf(pair)] = json
  }
  return out
})

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
  <div v-if="terminal.isPending.value" class="py-16 text-center text-sm text-muted-foreground">
    {{ t('store.loading') }}
  </div>
  <div v-else-if="terminal.isError.value" class="py-16 text-center text-sm text-destructive">
    {{ codeErrorMessage(parseError(terminal.error.value), t) }}
  </div>

  <div v-else class="grid min-h-[32rem] gap-3 @container lg:grid-cols-[14rem_1fr]" data-tour="exchange-terminal">
    <aside class="flex flex-col gap-2 rounded-lg border border-border p-2">
      <form class="grid gap-1" @submit.prevent="adder.submit(add, $event)">
        <div class="flex items-center gap-1">
          <div class="relative min-w-0 flex-1" :data-invalid="adder.invalidAttr('pair')">
            <Search class="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              v-model="draft"
              class="h-8 pl-7 uppercase"
              :placeholder="t('trading.symbol')"
              :aria-label="t('trading.symbol')"
            />
          </div>
          <Button type="submit" variant="ghost" size="icon" class="size-8" :title="t('terminal.add')" :aria-label="t('terminal.add')">
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
            class="invisible text-muted-foreground hover:text-destructive group-hover:visible focus-visible:visible"
            :aria-label="t('terminal.remove', { symbol: pair })"
            @click="removeFromList(pair)"
          >
            <X class="size-3.5" />
          </Button>
        </li>
        </ContextMenu>
      </ul>
      <PriceAlertList :store-id="props.storeId" class="mt-auto border-t border-border pt-2" />
    </aside>

    <TradingChart
      :key="`${props.storeId}:${chartNonce}`"
      :symbol="baseOf(active || 'BTCUSDT')"
      :saved-drawings="savedDrawings"
      :timeframe="terminal.data.value?.timeframe ?? '1h'"
      :indicators="indicatorKeys.length ? indicatorKeys : ['sma']"
      class="min-h-[30rem]"
      can-alert
      @alert="onAlert"
      @drawings="onDrawings"
      @timeframe="onTimeframe"
      @indicators="onIndicators"
    />
    <PriceAlertDialog v-model:open="alertOpen" :store-id="props.storeId" :preset="alertPreset" />
  </div>
</template>
