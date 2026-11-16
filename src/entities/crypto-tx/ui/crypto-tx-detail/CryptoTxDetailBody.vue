<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check } from 'lucide-vue-next'

import { Money } from '@/entities/currency/@x/crypto-tx'
import { Badge } from '@/shared/ui/badge'
import { formatDateTime, formatDay } from '@/shared/lib/format'

import { legAmount, opPlace, opTitle, shortAddress } from '../../lib/op-view'
import { useCryptoPairs } from '../../model/use-crypto-pairs'
import { CryptoPairNote } from '../crypto-pair-note'
import type { CryptoTxDetailProps } from './crypto-tx-detail.types'

/**
 * The read-only card of one operation: what the network said, and the chain it is in. Amounts, rate and
 * balance are shown, never edited. The actions (the meaning, the exchange, the explorer link) are the
 * owner's, in `#actions`; the address of the explorer page is not shown or fetched here.
 */
const props = defineProps<CryptoTxDetailProps>()
defineSlots<{ actions?(): unknown }>()

const { t } = useI18n()
const row = computed(() => props.detail.row)
/** The fiat record this operation was pointed at (spec E13): a reference, never a sum. */
const pairs = useCryptoPairs(() => ({ event_id: props.detail.row.id }))
const reclassified = computed(() =>
  t('cryptoTrack.operations.detail.reclassified', { kind: t(`cryptoTrack.operations.kind.${row.value.kind}`) }),
)
const title = computed(() => opTitle(row.value, t))
const facts = computed(() => {
  const list: [string, string][] = [
    [t('cryptoTrack.operations.detail.wallet'), `${row.value.account_name} · ${opPlace(row.value)}`],
  ]
  if (row.value.from) list.push([t('cryptoTrack.operations.detail.sender'), row.value.from])
  if (row.value.to) list.push([t('cryptoTrack.operations.detail.recipient'), row.value.to])
  if (row.value.reference) {
    const label = t(row.value.exchange ? 'cryptoTrack.operations.detail.fill' : 'cryptoTrack.operations.detail.hash')
    list.push([label, props.detail.url_note ?? row.value.reference])
  }
  if (props.detail.method) list.push([t('cryptoTrack.operations.detail.method'), props.detail.method])
  if (row.value.protocol) list.push([t('cryptoTrack.operations.detail.protocol'), row.value.protocol])
  if (row.value.contact_name) {
    const guess = row.value.contact_guessed ? ` (${t('cryptoTrack.operations.detail.byAddress')})` : ''
    list.push([t('cryptoTrack.operations.detail.through'), row.value.contact_name + guess])
  }
  if (row.value.categories.length) list.push([t('cryptoTrack.operations.detail.category'), row.value.categories.join(', ')])
  if (row.value.comment) list.push([t('cryptoTrack.operations.detail.description'), row.value.comment])
  return list
})
/** The rule in its own words: what it decides, so «remove» is pressed knowing what goes. */
const ruleText = computed(() => {
  const rule = props.detail.contract_rule
  if (!rule) return ''
  const parts = [
    rule.kind ? t(`cryptoTrack.operations.kind.${rule.kind}`) : '',
    rule.categories.length ? rule.categories.join(', ') : '',
  ].filter(Boolean)
  return parts.length ? parts.join(' · ') : t('cryptoTrack.operations.detail.ruleContactOnly')
})
const exchangeSide = (member: CryptoChainMember) => t(member.side === 'fiat' ? 'cryptoTrack.operations.detail.fiat' : 'cryptoTrack.operations.detail.thisOne')
</script>

<template>
  <div class="grid gap-4">
    <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.operations.detail.readonly') }}</p>
    <div class="grid gap-1">
      <div class="flex flex-wrap items-center gap-2">
        <Badge v-if="row.flow_id" variant="secondary">{{ t('cryptoTrack.operations.exchange') }}</Badge>
        <Badge v-if="row.hidden" variant="outline">{{ t('cryptoTrack.operations.hiddenBadge') }}</Badge>
        <Badge v-if="row.kind !== row.source_kind" variant="outline">{{ reclassified }}</Badge>
      </div>
      <p class="sr-only">{{ title }}</p>
      <p
        v-for="leg in row.legs"
        :key="leg.asset_key + leg.direction"
        class="text-2xl font-semibold tabular-nums"
        :class="leg.direction === 'in' ? 'text-success' : 'text-destructive'"
      >
        <Money :amount="legAmount(leg)" :currency="leg.symbol" :at="row.time" signed :convert="false" />
      </p>
      <p class="text-sm text-muted-foreground">{{ formatDay(row.time) }} · {{ formatDateTime(row.time).split(' ').at(-1) }}</p>
    </div>

    <!-- An own entry is a number the user cannot find in any chain: say where it came from, in place. -->
    <p v-if="row.own_entry" class="rounded-lg bg-muted p-3 text-sm" data-crypto-own-entry-note>
      {{ t(`cryptoTrack.operations.own.${row.own_entry}Note`) }}
    </p>
    <p v-if="detail.contract_rule" class="rounded-lg bg-muted p-3 text-sm" data-crypto-contract-rule>
      <span class="font-medium">{{ t('cryptoTrack.operations.detail.rule') }}</span>
      {{ ' ' }}{{ ruleText }}
      <span v-if="detail.contract" class="block text-xs text-muted-foreground">{{ shortAddress(detail.contract) }}</span>
    </p>

    <dl class="grid gap-2 border-t border-border pt-3 text-sm">
      <div v-for="[label, value] in facts" :key="label" class="flex items-baseline justify-between gap-4">
        <dt class="shrink-0 text-muted-foreground">{{ label }}</dt>
        <dd class="min-w-0 break-all text-right" :title="value">{{ value.startsWith('0x') ? shortAddress(value) : value }}</dd>
      </div>
      <div v-if="row.gas && row.gas_symbol" class="flex items-baseline justify-between gap-4">
        <dt class="shrink-0 text-muted-foreground">{{ t('cryptoTrack.operations.detail.gas') }}</dt>
        <dd class="text-right tabular-nums"><Money :amount="row.gas.amount" :currency="row.gas_symbol" :convert="false" /></dd>
      </div>
    </dl>

    <section v-if="detail.chain.length" class="grid gap-2" data-crypto-chain>
      <h4 class="text-sm font-medium text-muted-foreground">{{ t('cryptoTrack.operations.detail.chain') }}</h4>
      <ul class="divide-y divide-border rounded-lg border">
        <li v-for="member in detail.chain" :key="member.id" class="flex items-center justify-between gap-3 px-3 py-2" :class="member.is_this && 'bg-muted/50'">
          <span class="min-w-0">
            <span class="block font-medium tabular-nums"><Money :amount="member.amount" :currency="member.currency" signed :convert="false" /></span>
            <span class="block truncate text-xs text-muted-foreground">{{ member.place }} · {{ formatDay(member.time) }}</span>
          </span>
          <span class="shrink-0 text-xs text-muted-foreground">{{ member.is_this ? t('cryptoTrack.operations.detail.thisOne') : exchangeSide(member) }}</span>
        </li>
      </ul>
      <p v-if="row.gas" class="rounded-lg bg-muted p-3 text-xs text-muted-foreground">{{ t('cryptoTrack.operations.detail.gasNote') }}</p>
      <p v-if="detail.basis_from_chain" class="flex items-start gap-2 rounded-lg bg-muted p-3 text-sm">
        <Check class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
        {{ t('cryptoTrack.operations.detail.basisFromChain') }}
      </p>
    </section>

    <section v-if="pairs.data.value?.length" class="grid gap-2" data-tour="crypto-pair" data-crypto-pairs>
      <h4 class="text-sm font-medium text-muted-foreground">{{ t('cryptoTrack.links.cardTitle') }}</h4>
      <CryptoPairNote v-for="pair in pairs.data.value" :key="pair.fiat_tx_id" :pair="pair" show="fiat" />
    </section>

    <div v-if="$slots.actions" class="flex flex-wrap gap-2 border-t border-border pt-3"><slot name="actions" /></div>
  </div>
</template>
