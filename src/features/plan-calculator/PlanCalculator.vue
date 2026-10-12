<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { AssetInput } from '@/entities/asset'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError } from '@/shared/api/ipc'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
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
        at: null,
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

const form = useFormValidation(() => ({ amount: positiveAmount(amount.value), priceIn: positiveAmount(priceIn.value) }))

const CALC_MODES: CalcMode[] = ['converter', 'position']

function pickMode(value: unknown) {
  if (value === 'converter' || value === 'position') mode.value = value
}

/** Checks the two fields a plan needs, then hands the plan over. */
function apply(event?: Event) {
  form.submit(emitPlan, event)
}

function emitPlan() {
  emit('apply', {
    mode: mode.value,
    from: from.value,
    to: to.value,
    amount: amount.value.trim(),
    price: priceIn.value.trim(),
  })
}

/** The dialog owns the "apply" button, so it reads these. */
defineExpose({ apply })
</script>

<template>
  <div class="flex flex-col gap-4">
    <ToggleGroup
      type="single"
      variant="outline"
      size="sm"
      :model-value="mode"
      :aria-label="t('calc.title')"
      @update:model-value="pickMode"
    >
      <ToggleGroupItem v-for="tab in CALC_MODES" :key="tab" :value="tab">
        {{ t(`calc.mode.${tab}`) }}
      </ToggleGroupItem>
    </ToggleGroup>

    <div class="grid gap-3 @sm:grid-cols-2">
      <AssetInput v-model="from" :label="t('calc.from')" :placeholder="t('asset.searchPlaceholder')" />
      <AssetInput v-if="mode === 'converter'" v-model="to" :label="t('calc.to')" :placeholder="t('asset.searchPlaceholder')" />
      <div v-else class="grid gap-1.5">
        <Label>{{ t('calc.targetPrice') }}</Label>
        <AmountInput v-model="targetPrice" :placeholder="t('calc.targetPriceHint')" />
      </div>
    </div>

    <div class="grid gap-3 @sm:grid-cols-3">
      <FormField
        for="calc-amount"
        :label="t(mode === 'converter' ? 'calc.amount' : 'calc.quantity')"
        required
        :error="form.error('amount')"
      >
        <AmountInput id="calc-amount" v-model="amount" :placeholder="t('calc.amountHint')" />
      </FormField>
      <FormField
        for="calc-price-in"
        :label="t(mode === 'converter' ? 'calc.priceIn' : 'calc.entryPrice')"
        required
        :error="form.error('priceIn')"
      >
        <AmountInput id="calc-price-in" v-model="priceIn" placeholder="0" />
      </FormField>
      <div class="grid gap-1.5">
        <Label>{{ t(mode === 'converter' ? 'calc.priceOut' : 'calc.stopPrice') }}</Label>
        <AmountInput v-if="mode === 'converter'" v-model="priceOut" placeholder="0" />
        <AmountInput v-else v-model="stopPrice" :placeholder="t('calc.stopPriceHint')" />
      </div>
    </div>

    <div class="grid gap-1.5">
      <Label>{{ t('calc.feePct') }}</Label>
      <AmountInput v-model="feePct" placeholder="0.1" class="max-w-28" />
    </div>

    <p v-if="impliedRate && mode === 'converter'" class="text-xs text-muted-foreground">
      {{ t('calc.rate') }}: 1 {{ from?.symbol.toUpperCase() || '—' }} ≈ {{ impliedRate }}
      {{ to?.symbol.toUpperCase() || '' }}
    </p>

    <div v-if="rows.length" class="flex flex-col gap-1.5 rounded-md border border-border bg-muted/40 p-3">
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
  </div>
</template>
