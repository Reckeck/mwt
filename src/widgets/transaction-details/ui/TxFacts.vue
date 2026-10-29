<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { ReportTerm } from '@/entities/report'
import { EntityLink } from '@/shared/ui/entity-link'
import { formatPct } from '@/shared/lib/format'
import { txForMonthLabel, txMeta, txRouteSides } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { pairRate, txBaseValue, txFlowKind, txPair } from '../lib/tx-card'

/**
 * `From wallet · To wallet · Rate · Via · In base currency` (Paths 1 · 6):
 * every value leads on — a wallet to its glance (the `side` slot), the
 * counterparty to its card, `not spending` to what the word means.
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

const sides = computed(() => txRouteSides(props.tx))
const contact = computed(() => txMeta(props.tx).contact_id)
const at = computed(() => txMeta(props.tx).timestamp)

const rate = computed(() => {
  const pair = txPair(props.tx)
  if (!pair) return null
  return { price: pairRate(pair), currency: pair.out.symbol, asset: pair.into.symbol }
})
const compare = computed(() => {
  const delta = props.delta
  if (delta === null) return ''
  const pct = formatPct(Math.abs(delta), { decimals: 1 })
  if (Math.abs(delta) < 0.05) return t('txCard.rateSame')
  return t(delta > 0 ? 'txCard.rateAbove' : 'txCard.rateBelow', { pct })
})

const forMonth = computed(() => txForMonthLabel(props.tx))
const flow = computed(() => txFlowKind(props.tx))
const base = computed(() => {
  if (!props.baseCurrency) return undefined
  const value = txBaseValue(props.tx, Number(props.fxPerUsd ?? 1))
  return value === null ? null : { amount: value, currency: props.baseCurrency }
})

/** A wallet is `From wallet / To wallet`; a person or free text is who paid or who gets it. */
const fromLabel = computed(() => t(sides.value.from && 'store' in sides.value.from ? 'txCard.from' : 'txCard.payer'))
const toLabel = computed(() => t(sides.value.to && 'store' in sides.value.to ? 'txCard.to' : 'txCard.payee'))

const ROW = 'flex items-baseline justify-between gap-4 py-3'
</script>

<template>
  <dl class="grid divide-y divide-border border-y px-0 text-sm" data-tx-facts>
    <div v-if="sides.from" :class="ROW">
      <dt class="text-muted-foreground">{{ fromLabel }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <slot name="side" :side="sides.from" :label="context.sideLabel(sides.from)">{{ context.sideLabel(sides.from) }}</slot>
      </dd>
    </div>
    <div v-if="sides.to" :class="ROW">
      <dt class="text-muted-foreground">{{ toLabel }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <slot name="side" :side="sides.to" :label="context.sideLabel(sides.to)">{{ context.sideLabel(sides.to) }}</slot>
      </dd>
    </div>
    <div v-if="rate" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.rate') }}</dt>
      <dd class="text-right tabular-nums" data-tx-rate>
        <i18n-t keypath="txCard.ratePer" tag="span">
          <template #price><Money :amount="rate.price" :currency="rate.currency" :decimals="2" :at="at" /></template>
          <template #asset>{{ rate.asset }}</template>
        </i18n-t><span v-if="compare" class="text-muted-foreground"> · {{ compare }}</span>
      </dd>
    </div>
    <div v-if="contact" :class="ROW">
      <dt class="text-muted-foreground">{{ rate ? t('txCard.via') : t('txCard.counterparty') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <EntityLink :target="{ kind: 'contact', id: contact }" :label="context.contactName(contact)" />
      </dd>
    </div>
    <div v-if="forMonth" :class="ROW" data-tx-for-month>
      <dt class="text-muted-foreground">{{ t('txCard.forMonth') }}</dt>
      <dd class="text-right">{{ forMonth }}</dd>
    </div>
    <div v-if="base !== undefined" :class="ROW">
      <dt class="text-muted-foreground">{{ t('txCard.base') }}</dt>
      <dd class="text-right tabular-nums">
        <Money v-if="base" :amount="base.amount" :currency="base.currency" :at="at" />
        <template v-else>{{ t('txCard.noPrice') }}</template> ·
        <ReportTerm :label="t(`txCard.flow.${flow}`)" :hint="t(`txCard.flowHint.${flow}`)" />
      </dd>
    </div>
  </dl>
</template>
