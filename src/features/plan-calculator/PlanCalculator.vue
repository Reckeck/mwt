<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { cn } from '@/shared/lib/utils'

import type { CalculatorPlan, PlanCalculatorProps } from './plan-calculator.types'

const props = defineProps<PlanCalculatorProps>()

const emit = defineEmits<{
  /** Hand a calculated plan over to the transaction form. */
  apply: [payload: CalculatorPlan]
}>()

const { t } = useI18n()

const mode = ref<CalcMode>('converter')
const from = ref<AssetRef | null>(null)
const to = ref<AssetRef | null>(null)
const amount = ref('')
const priceIn = ref('')
const priceOut = ref('')
const targetPrice = ref('')
const stopPrice = ref('')
const feePct = ref('0.1')

const result = ref<CalcResult | null>(null)
const error = ref<string | null>(null)

const resultKey = computed(
  () => [mode.value, amount.value, priceIn.value, priceOut.value, targetPrice.value, stopPrice.value, feePct.value].join('|'),
)

let timer: ReturnType<typeof setTimeout> | undefined

watch(
  resultKey,
  () => {
    clearTimeout(timer)
    timer = setTimeout(() => void run(), 200)
  },
  { immediate: true },
)

async function run() {
  if (!amount.value.trim() || !priceIn.value.trim()) {
    result.value = null
    return
  }
  try {
    result.value = await call<CalcResult>('calc_preview', {
      input: {
        mode: mode.value,
        from_symbol: from.value?.symbol ?? '',
        to_symbol: to.value?.symbol ?? '',
        amount: amount.value.trim(),
        price_in: priceIn.value.trim(),
        price_out: priceOut.value.trim(),
        target_price: targetPrice.value.trim() || null,
        stop_price: stopPrice.value.trim() || null,
        fee_pct: feePct.value.trim() || '0',
      },
    })
    error.value = null
  } catch (err) {
    result.value = null
    error.value = parseError(err).message
  }
}

/** Pre-fill the entry price from the price cache when an asset is picked. */
async function autofillPrice(target: AssetRef | null, into: 'in' | 'out') {
  if (!target) return
  try {
    const res = await call<CachedPrice>('get_cached_price', {
      input: {
        symbol: target.symbol,
        coingecko_id: target.coingecko_id,
        chain: target.chain,
        contract: target.contract,
      },
    })
    if (res.ts) {
      if (into === 'in') priceIn.value = res.price_usd
      else priceOut.value = res.price_usd
    }
  } catch {
    // no cached quote — leave the field for manual input
  }
}

watch(from, (value) => void autofillPrice(value, 'in'))
watch(to, (value) => void autofillPrice(value, 'out'))

/** 1 unit of the source priced in the target, when both prices are known. */
const impliedRate = computed(() => {
  const priceInUsd = Number(priceIn.value)
  const priceOutUsd = Number(priceOut.value)
  if (!priceInUsd || !priceOutUsd) return null
  return (priceInUsd / priceOutUsd).toLocaleString(undefined, { maximumFractionDigits: 8 })
})

interface ResultRow {
  key: string
  label: string
  value: string
  emphasis?: boolean
  tone?: 'positive' | 'negative'
}

const rows = computed<ResultRow[]>(() => {
  const calc = result.value
  if (!calc) return []
  const base = props.baseCurrency
  const money = (value: string) => `${value} ${base.toUpperCase()}`
  if (mode.value === 'converter') {
    return [
      { key: 'gross_in', label: t('calc.grossIn'), value: money(calc.gross_in) },
      { key: 'fee', label: t('calc.fee'), value: money(calc.fee) },
      { key: 'gross_out', label: t('calc.grossOut'), value: money(calc.gross_out) },
      {
        key: 'net_out',
        label: t('calc.netOut'),
        value: `${calc.net_out} ${to.value?.symbol.toUpperCase() ?? ''}`.trim(),
        emphasis: true,
      },
    ]
  }
  return [
    { key: 'spent', label: t('calc.spent'), value: money(calc.spent) },
    {
      key: 'exit_value',
      label: t('calc.exitValue'),
      value: calc.exit_value === '0' ? '—' : money(calc.exit_value),
    },
    {
      key: 'pnl',
      label: t('calc.pnl'),
      value: money(calc.pnl),
      tone: Number(calc.pnl) < 0 ? 'negative' : 'positive',
    },
    {
      key: 'roi_pct',
      label: t('calc.roi'),
      value: calc.roi_pct === '0' ? '—' : `${calc.roi_pct}%`,
      tone: Number(calc.pnl) < 0 ? 'negative' : 'positive',
    },
    {
      key: 'risk',
      label: t('calc.risk'),
      value: calc.risk === '0' ? '—' : `${money(calc.risk)} (${calc.risk_pct}%)`,
      tone: 'negative',
    },
  ]
})

const canApply = computed(() => amount.value.trim() !== '' && priceIn.value.trim() !== '')

function apply() {
  emit('apply', {
    mode: mode.value,
    from: from.value,
    to: to.value,
    amount: amount.value.trim(),
    price: priceIn.value.trim(),
  })
}
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('calc.title') }}</p>
        <div class="flex rounded-md border border-border p-0.5">
          <button
            v-for="tab in (['converter', 'position'] as CalcMode[])"
            :key="tab"
            type="button"
            class="rounded px-2.5 py-1 text-xs font-medium transition-colors"
            :class="cn(mode === tab ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground')"
            @click="mode = tab"
          >
            {{ t(`calc.mode.${tab}`) }}
          </button>
        </div>
      </div>
    </template>

    <div class="grid gap-3">
      <div class="grid gap-3 @sm:grid-cols-2">
        <AssetInput v-model="from" :label="t('calc.from')" :placeholder="t('asset.searchPlaceholder')" />
        <AssetInput v-if="mode === 'converter'" v-model="to" :label="t('calc.to')" :placeholder="t('asset.searchPlaceholder')" />
        <div v-else class="grid gap-1.5">
          <Label>{{ t('calc.targetPrice') }}</Label>
          <Input v-model="targetPrice" inputmode="decimal" :placeholder="t('calc.targetPriceHint')" />
        </div>
      </div>

      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label>{{ t(mode === 'converter' ? 'calc.amount' : 'calc.quantity') }}</Label>
          <Input v-model="amount" inputmode="decimal" :placeholder="t('calc.amountHint')" />
        </div>
        <div class="grid gap-1.5">
          <Label>{{ t(mode === 'converter' ? 'calc.priceIn' : 'calc.entryPrice') }}</Label>
          <Input v-model="priceIn" inputmode="decimal" placeholder="0" />
        </div>
        <div class="grid gap-1.5">
          <Label>{{ t(mode === 'converter' ? 'calc.priceOut' : 'calc.stopPrice') }}</Label>
          <Input v-if="mode === 'converter'" v-model="priceOut" inputmode="decimal" placeholder="0" />
          <Input v-else v-model="stopPrice" inputmode="decimal" :placeholder="t('calc.stopPriceHint')" />
        </div>
      </div>

      <div class="grid gap-1.5">
        <Label>{{ t('calc.feePct') }}</Label>
        <Input v-model="feePct" inputmode="decimal" placeholder="0.1" class="max-w-28" />
      </div>

      <p v-if="impliedRate && mode === 'converter'" class="text-xs text-muted-foreground">
        {{ t('calc.rate') }}: 1 {{ from?.symbol.toUpperCase() || '—' }} ≈ {{ impliedRate }}
        {{ to?.symbol.toUpperCase() || '' }}
      </p>

      <div v-if="rows.length" class="space-y-1.5 rounded-md border border-border bg-muted/40 p-3">
        <div v-for="row in rows" :key="row.key" class="flex items-baseline justify-between gap-3 text-sm">
          <span class="text-muted-foreground">{{ row.label }}</span>
          <span
            class="truncate font-medium tabular-nums"
            :class="cn(
              row.emphasis && 'text-base',
              row.tone === 'positive' && 'text-emerald-600 dark:text-emerald-400',
              row.tone === 'negative' && 'text-destructive',
            )"
          >
            {{ row.value }}
          </span>
        </div>
      </div>

      <p v-if="error" class="text-xs text-destructive">{{ error }}</p>

      <Button variant="outline" size="sm" :disabled="!canApply" @click="apply">
        {{ t('calc.apply') }}
      </Button>
    </div>
  </Card>
</template>
