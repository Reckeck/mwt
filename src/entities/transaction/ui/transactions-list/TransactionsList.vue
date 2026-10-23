<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { Link } from 'lucide-vue-next'

import { ActionMenu, ActionSheet } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { txExplorerUrl, txId } from '@/shared/lib/transactions'
import { provideTxContext } from '@/shared/lib/tx-context'
import { useIsPhone } from '@/shared/lib/phone'

import { dayLabel } from '../../lib/day-label'
import { type TxDay, groupByDay } from '../../lib/group-by-day'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import type { TransactionsListProps } from './transactions-list.types'
import { useRowMenu } from './use-row-menu'

const props = withDefaults(defineProps<TransactionsListProps>(), {
  storeId: null,
  losses: null,
  openable: false,
  rowActions: null,
  groupByDay: false,
  fxPerUsd: null,
  baseCurrency: undefined,
  total: null,
  loadingMore: false,
  selection: null,
})
const emit = defineEmits<{ open: [tx: Transaction, list: Transaction[]]; more: [] }>()
/** A side's name in a row («Тинькофф → Bybit») and «авто», handed down to each row. */
const slots = defineSlots<{
  side?(props: { side: AccountRef; label: string }): unknown
  origin?(props: { tx: Transaction; automationId: string; runId: string | null }): unknown
}>()

const { t } = useI18n()
const phone = useIsPhone()

/**
 * A long history renders a page at a time: rows differ in height (badges,
 * comments, fees), so a fixed-height virtual list would jump; «Показать ещё»
 * adds the next page. A new filter starts from the first page again.
 */
const PAGE = 100
const limit = ref(PAGE)
watch(
  () => props.transactions,
  () => {
    limit.value = PAGE
  },
)
/** Paged by the backend: every row it sent is shown. */
const paged = computed(() => props.total !== null)
const shown = computed(() => (paged.value ? props.transactions : props.transactions.slice(0, limit.value)))
const context = provideTxContext()
/** Without grouping: one day-less group, so the template has one shape. */
/** Picking on a phone: one flat list, each row names its day (Ops-1 · 4). */
const picking = computed(() => phone.value && Boolean(props.selection?.active.value))
const grouped = computed(() => props.groupByDay && !picking.value)
const days = computed<TxDay[]>(() =>
  grouped.value
    ? groupByDay(shown.value, Number(props.fxPerUsd ?? 1), context.rootOf, props.storeId)
    : [{ date: '', txs: shown.value, totalBase: 0, partial: false }],
)
/** The row's menu: the list's own actions, the explorer link (when there is one) just before the delete. */
function actionsFor(tx: Transaction): UiAction[] {
  const own = props.rowActions?.(tx) ?? []
  const url = txExplorerUrl(tx)
  if (!url) return own
  const explorer: UiAction = { id: 'explorer', label: t('hist.openExplorer'), icon: Link, run: () => void call('open_explorer', { url }) }
  const at = own.findIndex((action) => action.destructive)
  return at < 0 ? [...own, explorer] : [...own.slice(0, at), explorer, ...own.slice(at)]
}

const listRoot = ref<HTMLElement | null>(null)
const menuTx = ref<Transaction | null>(null)
const menuActions = computed(() => (menuTx.value ? actionsFor(menuTx.value) : []))
/** Outside a row (a day header, «Показать ещё») there is nothing to act on: no menu. */
function onContextMenu(event: MouseEvent) {
  // A finger holds for the sheet instead (useRowMenu); no floating menu under it.
  if (rowMenu.coarse.value) {
    event.preventDefault()
    return
  }
  const id = (event.target as Element | null)?.closest<HTMLElement>('[data-tx-row]')?.dataset.txRow
  const tx = id ? shown.value.find((entry) => txId(entry) === id) : undefined
  if (!tx) {
    event.preventDefault()
    event.stopPropagation()
    return
  }
  menuTx.value = tx
}
// Capture: it runs before the menu's own handler, so the menu opens on the right row.
useEventListener(listRoot, 'contextmenu', onContextMenu, { capture: true })
const rowMenu = useRowMenu({
  root: listRoot,
  rows: () => shown.value,
  actions: actionsFor,
  enabled: () => Boolean(props.rowActions),
  context,
})
/** A day's box: all its rows, some of them, none. */
function dayState(day: TxDay): boolean | 'indeterminate' {
  const picked = day.txs.filter((tx) => props.selection?.has(txId(tx))).length
  if (!picked) return false
  return picked === day.txs.length ? true : 'indeterminate'
}
function toggleDay(day: TxDay) {
  props.selection?.setMany(day.txs.map(txId), dayState(day) !== true)
}
/** A tap while picking on a phone picks; otherwise the row opens its card. */
function onOpen(row: Transaction) {
  if (picking.value) props.selection?.toggle(txId(row))
  else emit('open', row, props.transactions)
}
const rest = computed(() => (props.total ?? props.transactions.length) - shown.value.length)

function showMore() {
  if (paged.value) emit('more')
  else limit.value += PAGE
}
</script>

<template>
  <!-- One right-click menu for the whole list: the row under the cursor says which operation. -->
  <ContextMenu lazy :actions="menuActions" :disabled="!props.rowActions || rowMenu.coarse.value">
    <div>
    <!-- The menu trigger takes the outer ref: the listener needs one of its own. -->
    <div ref="listRoot" class="contents divide-y divide-border">
      <section v-for="day in days" :key="day.date" class="divide-y divide-border">
        <!-- Opaque, no backdrop-blur: a blur on every day header froze WebKitGTK (the Linux window) for good. -->
        <h3
          v-if="day.date"
          class="sticky top-0 z-[1] flex items-baseline justify-between gap-2 py-1.5 text-xs text-muted-foreground"
          :class="phone ? 'bg-card pt-3' : '-mx-4 bg-muted px-4'"
        >
          <span class="flex items-center gap-3">
            <span v-if="props.selection && !phone" class="flex w-4 shrink-0">
              <Checkbox
                :model-value="dayState(day)"
                :aria-label="t('bulk.pickDay')"
                data-tx-select-day
                @click.stop="toggleDay(day)"
              />
            </span>
            {{ dayLabel(`${day.date}T12:00:00`) }}
          </span>
          <span class="tabular-nums" :title="day.partial ? t('tx.dayTotalPartial') : t('tx.dayTotal')">
            <Money :amount="day.totalBase" :currency="props.baseCurrency" :decimals="2" signed />
            <template v-if="day.partial">*</template>
          </span>
        </h3>
        <div
          v-for="tx in day.txs"
          :key="txId(tx)"
          :data-tx-row="txId(tx)"
          class="flex gap-3"
          :class="props.selection?.has(txId(tx)) && (phone ? '-mx-3 bg-primary/12 px-3' : '-mx-4 bg-primary/12 px-4')"
        >
          <span v-if="props.selection && !phone" class="flex w-4 shrink-0 pt-4">
            <Checkbox
              :model-value="props.selection.has(txId(tx))"
              :aria-label="t('bulk.pick')"
              data-tx-select
              @click.stop="(event: MouseEvent) => props.selection?.toggle(txId(tx), event.shiftKey)"
            />
          </span>
          <TransactionRow
            class="min-w-0 flex-1"
            :tx="tx"
            :store-id="props.storeId"
            :loss="props.losses?.[txId(tx)] ?? null"
            :openable="props.openable"
            :dated="!grouped"
            :selected="picking ? props.selection?.has(txId(tx)) : null"
            @open="onOpen"
          >
            <!-- The same list as the right-click menu, behind «⋯» for a mouse and a finger alike. -->
            <template v-if="props.rowActions" #actions="{ tx: row }">
              <ActionMenu :actions="actionsFor(row)" :label="t('actions.more')" :sheet="rowMenu.sheetHead(row)" />
            </template>
            <!-- The clipped second line leaves a wallet name a 16px target on touch: a tap opens the card, a press — the menu with the wallet. -->
            <template v-if="slots.side && !rowMenu.coarse.value" #side="side">
              <slot name="side" v-bind="side" />
            </template>
            <template v-if="slots.origin" #origin="origin">
              <slot name="origin" v-bind="origin" />
            </template>
          </TransactionRow>
        </div>
      </section>

      <div v-if="rest > 0" class="flex justify-center py-3">
        <Button variant="outline" size="sm" :disabled="props.loadingMore" @click="showMore">
          {{ t('ui.showMore', { count: Math.min(rest, PAGE) }) }}
        </Button>
      </div>

      <p v-if="!props.transactions.length" class="py-6 text-center text-sm text-muted-foreground">
        {{ t('tx.empty') }}
      </p>
    </div>
    <ActionSheet
      v-if="rowMenu.sheetTx.value"
      v-model:open="rowMenu.sheetOpen.value"
      :actions="rowMenu.sheetActions.value"
      v-bind="rowMenu.sheetHead(rowMenu.sheetTx.value)"
    />
    </div>
  </ContextMenu>
</template>
