<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { ContextMenu } from '@/shared/ui/context-menu'
import { Money } from '@/entities/currency'
import { formatDay } from '@/shared/lib/format'
import { useCoarsePointer } from '@/shared/lib/phone'
import { txId } from '@/shared/lib/transactions'
import { provideTxContext } from '@/shared/lib/tx-context'

import { type TxDay, groupByDay } from '../../lib/group-by-day'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import type { TransactionsListProps } from './transactions-list.types'

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
})
const emit = defineEmits<{ open: [tx: Transaction]; more: [] }>()

defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()

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
const days = computed<TxDay[]>(() =>
  props.groupByDay
    ? groupByDay(shown.value, Number(props.fxPerUsd ?? 1), context.rootOf, props.storeId)
    : [{ date: '', txs: shown.value, totalBase: 0, partial: false }],
)
const coarse = useCoarsePointer()

const listRoot = ref<HTMLElement | null>(null)
const menuTx = ref<Transaction | null>(null)
const menuActions = computed(() => (menuTx.value && props.rowActions ? props.rowActions(menuTx.value) : []))
/** Outside a row (a day header, «Показать ещё») there is nothing to act on: no menu. */
function onContextMenu(event: MouseEvent) {
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
const rest = computed(() => (props.total ?? props.transactions.length) - shown.value.length)

function showMore() {
  if (paged.value) emit('more')
  else limit.value += PAGE
}
</script>

<template>
  <!-- One right-click menu for the whole list: the row under the cursor says which operation. -->
  <ContextMenu lazy :actions="menuActions" :disabled="!props.rowActions">
    <div>
    <!-- The menu trigger takes the outer ref: the listener needs one of its own. -->
    <div ref="listRoot" class="contents divide-y divide-border">
      <section v-for="day in days" :key="day.date" class="divide-y divide-border">
        <!-- Opaque, no backdrop-blur: a blur on every day header froze WebKitGTK (the Linux window) for good. -->
        <h3
          v-if="day.date"
          class="sticky top-0 z-[1] flex items-baseline justify-between gap-2 bg-card py-1.5 text-xs font-medium text-muted-foreground"
        >
          <span class="capitalize">{{ formatDay(`${day.date}T12:00:00`) }}</span>
          <span class="tabular-nums" :title="day.partial ? t('tx.dayTotalPartial') : t('tx.dayTotal')">
            <Money :amount="day.totalBase" :currency="props.baseCurrency" :decimals="2" signed />
            <template v-if="day.partial">*</template>
          </span>
        </h3>
        <div v-for="tx in day.txs" :key="txId(tx)" :data-tx-row="txId(tx)">
          <TransactionRow
            :tx="tx"
            :store-id="props.storeId"
            :loss="props.losses?.[txId(tx)] ?? null"
            :openable="props.openable"
            @open="(row) => emit('open', row)"
          >
            <template #actions="{ tx: row }">
              <slot name="actions" :tx="row" />
              <!-- A finger has no right click: the same menu behind «⋯». -->
              <span v-if="coarse && props.rowActions?.(row).length">
                <ActionMenu :actions="props.rowActions(row)" :label="t('actions.more')" />
              </span>
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
    </div>
  </ContextMenu>
</template>
