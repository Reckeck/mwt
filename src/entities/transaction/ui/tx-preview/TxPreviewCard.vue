<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { AlertTriangle, ArrowRight } from 'lucide-vue-next'

import { formatMoney, numberFormat } from '@/shared/lib/format'
import { parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { TxPreviewProps } from './tx-preview.types'

const props = defineProps<TxPreviewProps>()
const { t } = useI18n()

/** Fiat with cents, crypto with up to 8 significant decimals and no trailing zeros. */
function qty(value: string, asset: AssetRef): string {
  const fiat = asset.chain === 'fiat'
  return numberFormat({ minimumFractionDigits: fiat ? 2 : 0, maximumFractionDigits: fiat ? 2 : 8 }).format(Number(value))
}

const rows = computed(() =>
  (props.preview?.changes ?? []).map((change) => {
    const delta = Number(change.after) - Number(change.before)
    const warned = props.preview?.warnings.some((warning) => warning.store_id === change.store_id) ?? false
    return { ...change, delta, warned }
  }),
)

const flowText = computed(() => {
  const flow = props.preview?.flow
  if (!flow || flow === 'ignored') return null
  return t(`txPreview.flow.${flow}`)
})

/**
 * A refusal is shown only when it is about money (binding, funds, limits); a
 * missing comment or category is the form's own business while it is filled in.
 */
const MONEY_CODES = new Set(['BINDING_MISMATCH', 'INSUFFICIENT_FUNDS', 'ARCHIVED', 'WALLET_NOT_APPLICABLE'])
const errorText = computed(() => {
  const raw = props.preview?.error
  if (!raw) return null
  const parsed = parseError(raw)
  return MONEY_CODES.has(parsed.code) ? codeErrorMessage(parsed, t) : null
})

// money: plain — a transient warning in the entity; an entity cannot draw a neighbour's <Money>
const warningTexts = computed(() =>
  (props.preview?.warnings ?? []).map((warning) => {
    const name = props.preview?.changes.find((change) => change.store_id === warning.store_id)?.store_name ?? ''
    return warning.kind === 'credit_limit'
      ? t('txPreview.creditLimit', { store: name, limit: formatMoney(warning.limit) })
      : t('txPreview.negative', { store: name, asset: warning.asset.symbol })
  }),
)
</script>

<template>
  <section
    v-if="props.preview && (rows.length || errorText)"
    class="sticky bottom-0 z-10 grid gap-2 rounded-md border border-border bg-card p-3 text-sm shadow-[0_-8px_16px_-12px_rgba(0,0,0,0.5)]"
    :aria-busy="props.loading"
    data-tour="tx-preview"
  >
    <p class="text-xs font-medium text-muted-foreground">{{ t('txPreview.title') }}</p>

    <ul v-if="rows.length" class="grid gap-1.5">
      <li v-for="row in rows" :key="`${row.store_id}:${row.asset.symbol}`" class="flex flex-wrap items-center gap-x-2 gap-y-0.5">
        <span class="min-w-0 flex-1 truncate">
          {{ row.store_name }} · <span class="uppercase text-muted-foreground">{{ row.asset.symbol }}</span>
        </span>
        <span class="tabular-nums text-muted-foreground">{{ qty(row.before, row.asset) }}</span>
        <ArrowRight class="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span class="font-medium tabular-nums" :class="row.warned && 'text-destructive'">{{ qty(row.after, row.asset) }}</span>
        <span class="w-24 text-right text-xs tabular-nums" :class="row.delta < 0 ? 'text-destructive' : 'text-success'">
          {{ row.delta > 0 ? '+' : '' }}{{ qty(String(row.delta), row.asset) }}
        </span>
      </li>
    </ul>

    <p v-if="flowText" class="text-xs text-muted-foreground">{{ flowText }}</p>

    <p v-for="text in warningTexts" :key="text" class="flex items-start gap-1.5 text-xs text-warning">
      <AlertTriangle class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
      {{ text }}
    </p>
    <p v-if="errorText" class="flex items-start gap-1.5 text-xs text-destructive">
      <AlertTriangle class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
      {{ errorText }}
    </p>
  </section>
</template>
