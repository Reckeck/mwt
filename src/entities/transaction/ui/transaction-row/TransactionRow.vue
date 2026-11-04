<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Money } from '@/entities/currency'
import { Check, ExternalLink, Paperclip } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { KeptMark } from '@/shared/ui/kept-mark'
import { EntityLink } from '@/shared/ui/entity-link'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { txForMonthLabel } from '@/shared/lib/transactions'
import {
  amountClass,
  txAutomation,
  txDebt,
  txDetails,
  txExplorerUrl,
  txFee,
  txHasNoPrice,
  txId,
  txIsInternal,
  txIsMove,
  txKind,
  txLegForStore,
  txMeta,
  txPrincipalLegs,
} from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'
import { useIsPhone } from '@/shared/lib/phone'

import { dayLabel } from '../../lib/day-label'
import { TX_KIND_ICON } from '../../lib/kind-icon'
import { routeRepeats, sideRoute, sideRouteText } from '../../lib/side-route'
import { useTxTitle } from '../../lib/tx-title'
import type { TransactionRowProps } from './transaction-row.types'

const props = withDefaults(defineProps<TransactionRowProps>(), { storeId: null, loss: null, openable: false, dated: true, selected: null })
const emit = defineEmits<{ open: [tx: Transaction] }>()

/**
 * The row's menu («⋯») comes from the list: entities do not own user actions.
 * Without one, the column holds the explorer link (or stays empty), so every
 * row of a list keeps the same width for its amounts (rule 17).
 */
const slots = defineSlots<{
  actions?(props: { tx: Transaction }): unknown
  /** A side's name in `Tinkoff → Bybit`: the list may make it a wallet's glance (Paths 1 · 4). */
  side?(props: { side: AccountRef; label: string }): unknown
  /** `Auto` of a booked one: the list may make it the automation's glance (Paths 2 · 10). */
  origin?(props: { tx: Transaction; automationId: string; runId: string | null }): unknown
}>()

const { t } = useI18n()
const context = useTxContext()
const phone = useIsPhone()

/** Money moved between the user's own wallets is neither earned nor spent: no green or red, one sum. */
const ownMove = computed(() => txIsMove(props.tx))
/** A same-asset «conversion» looks like the transfer it is. */
const kind = computed(() => (ownMove.value ? 'transfer' : txKind(props.tx)))
const titleOf = useTxTitle()
/** `Salary`, `Mom`, `RUB → USDT`, `Transfer`: the currency stays with the sum. */
const title = computed(() => titleOf(props.tx))
const badges = computed(() => meta.value.categories.filter((key) => key !== title.value.category))
const meta = computed(() => txMeta(props.tx))
const legs = computed(() => txPrincipalLegs(props.tx))
const attached = computed(() => context.attachmentCount(txId(props.tx)))
const internal = computed(() => txIsInternal(props.tx, context.rootOf))
const debt = computed(() => txDebt(props.tx))
const automation = computed(() => txAutomation(props.tx))
const fee = computed(() => txFee(props.tx))
/** `for September`: an income recorded for a month other than its date's. */
const forMonth = computed(() => txForMonthLabel(props.tx))
/** Brought in from an exchange; a correction has no categories, so it gets the mark here. */
const imported = computed(() => {
  const origin = meta.value.origin
  return typeof origin === 'object' && 'import' in origin && !meta.value.categories.includes('exchange_import')
})

/** Where the money went or came from, seen from this list's wallet; in parts so each side can be a link. */
const routeParts = computed(() => sideRoute(props.tx, context.sideLabel))
const route = computed(() => sideRouteText(routeParts.value))

/** In a wallet's history only its own side; in the full list every principal leg, a move's once. */
const shownLegs = computed(() => {
  if (props.storeId) return [txLegForStore(props.tx, props.storeId)].filter(Boolean)
  return ownMove.value ? legs.value.filter((leg) => Number(leg.amount) > 0).slice(0, 1) : legs.value
})
const explorer = computed(() => txExplorerUrl(props.tx))

function openExplorer() {
  if (explorer.value) void call('open_explorer', { url: explorer.value })
}

/** `Deal rate 93.75 · via skycapital`: the facts and the comment on one line. */
const notes = computed(() =>
  [...txDetails(props.tx).map(([key, value]) => `${t(key)} ${value}`), commentNotInRoute.value].filter(Boolean).join(' · '),
)

const commentNotInRoute = computed(() => (routeRepeats(routeParts.value, meta.value.comment, where.value, title.value.text) ? '' : meta.value.comment))
const where = computed(() => context.storeName(shownLegs.value[0]?.store_id ?? ''))
/** The phone's one line: the comment first, then where; picking, the day instead of where. */
const phoneLine = computed(() => {
  const place = route.value || where.value
  const comment = commentNotInRoute.value
  if (props.selected === null) return [comment, place]
  return comment ? [comment, dayLabel(meta.value.timestamp)] : [dayLabel(meta.value.timestamp), place]
})
</script>

<template>
  <div class="flex gap-x-3 gap-y-1 py-3" :class="phone ? 'items-center' : 'flex-wrap items-start'">
    <!-- On the phone a square mark instead of the word, so the amount keeps its line. -->
    <span v-if="phone && props.selected !== null" class="flex size-9 shrink-0 items-center justify-center">
      <span
        class="flex size-5 items-center justify-center rounded-full"
        :class="props.selected ? 'bg-primary text-primary-foreground' : 'border border-muted-foreground'"
      >
        <Check v-if="props.selected" class="size-3.5" />
      </span>
    </span>
    <!-- One grey icon for every kind, PC and phone: a word mark (`Correction`) overflowed its column.
         The colour of the amount already tells in from out; the kind's name is in the title. -->
    <span
      v-else
      class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground"
      :title="t(`tx.kindNoun.${kind}`)"
      data-tx-kind
    >
      <component :is="TX_KIND_ICON[kind]" class="size-4" aria-hidden="true" />
      <span class="sr-only">{{ t(`tx.kindNoun.${kind}`) }}</span>
    </span>

    <!-- An openable row opens from a see-through button over its body; the wallet names sit above it. -->
    <div class="relative min-w-0 flex-1" :class="!phone && 'basis-56'">
      <Button
        v-if="props.openable"
        variant="ghost"
        size="block"
        class="absolute inset-0 rounded-sm p-0 hover:bg-transparent"
        :title="t('txDetails.open')"
        :aria-label="t('txDetails.open')"
        data-tx-open
        @click="emit('open', props.tx)"
      />
      <div class="pointer-events-none relative flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span class="font-medium" data-tx-title>{{ title.text }}</span>
        <span
          v-if="attached"
          class="flex shrink-0 items-center gap-0.5 text-xs text-muted-foreground"
          :aria-label="t('attachments.count', { count: attached })"
          :title="t('attachments.count', { count: attached })"
          data-tx-attachments
        >
          <Paperclip class="size-3" aria-hidden="true" />{{ attached }}
        </span>
        <KeptMark v-if="meta.keep" />
        <!-- Links sit above the see-through button; the row's own menu lists them (no menu of their own). -->
        <Badge v-for="key in badges" :key="key" variant="outline" class="pointer-events-auto relative min-w-0">
          <EntityLink :target="{ kind: 'category', id: key }" :label="context.categoryName(key)" plain :menu="false" />
        </Badge>
        <Badge v-if="forMonth" variant="outline" class="shrink-0" data-tx-for-month>{{ t('tx.forMonth', { month: forMonth }) }}</Badge>
        <Badge v-if="imported" variant="outline" class="shrink-0">{{ t('tx.imported') }}</Badge>
        <span v-if="automation" class="pointer-events-auto relative shrink-0" data-tx-auto>
          <slot name="origin" :tx="props.tx" :automation-id="automation.id" :run-id="automation.runId">
            <Badge variant="outline">{{ t('glance.auto.badge') }}</Badge>
          </slot>
        </span>
        <Badge v-if="internal" variant="secondary" class="shrink-0">{{ t('tx.internal') }}</Badge>
        <Badge v-if="debt" variant="secondary" class="shrink-0">{{ t(`debt.${debt}`) }}</Badge>
        <Badge v-if="txHasNoPrice(props.tx)" variant="outline" class="shrink-0 text-muted-foreground">
          {{ t('tx.noPrice') }}
        </Badge>
      </div>
      <!-- The phone has one line under the title: the comment first, then where the money went. -->
      <p v-if="phone" class="pointer-events-none relative truncate text-xs text-muted-foreground">
        {{ phoneLine.filter(Boolean).join(' · ') }}
      </p>
      <p v-else class="pointer-events-none relative truncate text-xs text-muted-foreground">
        <template v-if="props.dated">{{ formatDateTime(meta.timestamp) }} · </template>
        <!-- A two-sided route already names the wallet. -->
        <template v-if="!route.includes('→') || route.startsWith('→')">{{ `${where} ` }}</template>
        <template v-if="route">
          <template v-for="(part, index) in routeParts" :key="index">
            <template v-if="typeof part === 'string'">{{ part }} </template>
            <span v-else class="pointer-events-auto relative">
              <slot name="side" :side="part.side" :label="part.label">{{ part.label }}</slot>
            </span>
            {{ ' ' }}
          </template>
        </template>
        <template v-if="meta.contact_id">
          {{ ' · ' }}
          <span class="pointer-events-auto relative">
            <EntityLink
              :target="{ kind: 'contact', id: meta.contact_id }"
              :label="context.contactName(meta.contact_id)"
              plain
              :menu="false"
              class="text-muted-foreground"
            />
          </span>
        </template>
      </p>
      <p v-if="notes && !phone" class="pointer-events-none relative truncate text-xs text-muted-foreground">{{ notes }}</p>
    </div>

    <!-- On a narrow screen the amounts and actions wrap under the text instead of squeezing it. -->
    <div class="ml-auto shrink-0 text-right">
      <p
        v-for="leg in shownLegs"
        :key="leg.store_id + leg.asset.symbol"
        class="whitespace-nowrap font-medium tabular-nums"
        :class="ownMove ? 'text-foreground' : amountClass(leg.amount)"
      >
        <Money :amount="leg.amount" :currency="leg.asset.symbol" :at="meta.timestamp" :signed="!ownMove || Boolean(props.storeId)" />
      </p>
      <p v-if="fee" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
        {{ t('tx.fee') }} <Money :amount="fee.amount" :currency="fee.asset" />
      </p>
      <p
        v-if="props.loss && Number(props.loss.total) > 0"
        class="whitespace-nowrap text-xs text-warning tabular-nums"
        :title="t('losses.rowHint')"
      >
        {{ t('losses.rowLabel') }} <Money :amount="props.loss.total" :currency="props.loss.currency" />
      </p>
    </div>

    <!-- One column of fixed width in every row: nothing moves on hover, amounts line up. -->
    <!-- On the phone the menu is a hold on the row (the list's sheet): the amount takes the room. -->
    <div v-if="!phone" class="flex w-8 shrink-0 justify-end">
      <slot v-if="slots.actions" name="actions" :tx="props.tx" />
      <Button
        v-else-if="explorer"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground"
        :title="t('hist.openExplorer')"
        :aria-label="t('hist.openExplorer')"
        @click="openExplorer"
      >
        <ExternalLink class="size-4" />
      </Button>
    </div>
  </div>
</template>
