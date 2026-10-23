<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ReportTerm } from '@/entities/report'
import { EntityLink } from '@/shared/ui/entity-link'
import { formatMoney, formatPct } from '@/shared/lib/format'
import { txMeta, txSides } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { pairRate, txBaseValue, txFlowKind, txPair } from '../lib/tx-card'

/**
 * «Из кошелька · В кошелёк · Курс · Через · В базовой валюте» (Paths 1 · 6):
 * every value leads on — a wallet to its glance (the `side` slot), the
 * counterparty to its card, «не трата» to what the word means.
 */
const props = defineProps<{
  tx: Transaction
  /** The rate against past exchanges, in percent. */
  delta: number | null
  fxPerUsd?: string | number
  baseCurrency?: string
}>()

defineSlots<{ side?(props: { side: AccountRef; label: string }): unknown }>()

const { t } = useI18n()
const context = useTxContext()

const sides = computed(() => txSides(props.tx))
const contact = computed(() => txMeta(props.tx).contact_id)

const rate = computed(() => {
  const pair = txPair(props.tx)
  if (!pair) return null
  const price = formatMoney(pairRate(pair), { currency: pair.out.symbol, decimals: 2 })
  return t('txCard.ratePer', { price, asset: pair.into.symbol })
})
const compare = computed(() => {
  const delta = props.delta
  if (delta === null) return ''
  const pct = formatPct(Math.abs(delta), { decimals: 1 })
  if (Math.abs(delta) < 0.05) return t('txCard.rateSame')
  return t(delta > 0 ? 'txCard.rateAbove' : 'txCard.rateBelow', { pct })
})

const flow = computed(() => txFlowKind(props.tx))
const base = computed(() => {
  if (!props.baseCurrency) return null
  const value = txBaseValue(props.tx, Number(props.fxPerUsd ?? 1))
  return value === null ? t('txCard.noPrice') : formatMoney(value, { currency: props.baseCurrency })
})

const ROW = 'flex items-baseline justify-between gap-4 py-3'
</script>

<template>
  <dl class="grid divide-y divide-border border-y px-0 text-sm" data-tx-facts>
    <div v-if="sides.from" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.from') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <slot name="side" :side="sides.from" :label="context.sideLabel(sides.from)">{{ context.sideLabel(sides.from) }}</slot>
      </dd>
    </div>
    <div v-if="sides.to" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.to') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <slot name="side" :side="sides.to" :label="context.sideLabel(sides.to)">{{ context.sideLabel(sides.to) }}</slot>
      </dd>
    </div>
    <div v-if="rate" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.rate') }}</dt>
      <dd class="text-right tabular-nums" data-tx-rate>
        {{ rate }}<span v-if="compare" class="text-muted-foreground"> · {{ compare }}</span>
      </dd>
    </div>
    <div v-if="contact" :class="ROW">
      <dt class="text-muted-foreground">{{ rate ? t('txCard.via') : t('txCard.counterparty') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <EntityLink :target="{ kind: 'contact', id: contact }" :label="context.contactName(contact)" />
      </dd>
    </div>
    <div v-if="base" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.base') }}</dt>
      <dd class="text-right tabular-nums">
        {{ base }} ·
        <ReportTerm :label="t(`txCard.flow.${flow}`)" :hint="t(`txCard.flowHint.${flow}`)" />
      </dd>
    </div>
  </dl>
</template>
