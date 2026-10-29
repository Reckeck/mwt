<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { ArrowDown, Star } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { ContextMenu } from '@/shared/ui/context-menu'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { currencySymbol, formatDate, formatDayMonthShort, formatPct } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { walletSubtitle } from '@/entities/store'
import { useWalletActions } from '../model/use-wallet-actions'
import { useWalletExpansion } from '../model/use-wallet-expansion'
import type { WalletRowEvents, WalletSort } from '../stores.types'
import WalletExpand from './WalletExpand.vue'

/**
 * The wallets as a table (Wallets-1 · 1, Currency-1 · 2): the star, the wallet
 * with its kind, what it holds in its own currencies, the last operation, the
 * sum. The name and the sum headers order the rows; a click on a row opens it.
 * A wallet with sub-accounts has a chevron: unfolded, they follow as indented
 * rows (28 px icon, the share in place of the last operation, no star or menu).
 */
const props = defineProps<{ rows: StoreTreeNode[]; currency: string; shown: (amount: number) => number | null }>()
const emit = defineEmits<WalletRowEvents>()
const sort = defineModel<WalletSort>('sort', { required: true })
const { t } = useI18n()
const expansion = useWalletExpansion()
const { actions, pinned, togglePin } = useWalletActions(
  (store) => emit('toggleArchive', store),
  (store) => emit('delete', store),
)

/** Header cells as on the frame: 12px, muted. */
const TH = 'h-9 px-3 text-xs font-medium text-muted-foreground'
const LINK_CLASS = [
  'block truncate font-medium after:absolute after:inset-0',
  'focus-visible:outline-none focus-visible:after:ring-2 focus-visible:after:ring-inset focus-visible:after:ring-ring',
]
const ariaSort = (key: WalletSort) => (sort.value === key ? (key === 'name' ? 'ascending' : 'descending') : 'none')
/** `Oct 26` this year, with the year (as the full date) for any other. */
const lastDay = (iso: string) =>
  new Date(iso).getFullYear() === new Date().getFullYear() ? formatDayMonthShort(iso) : formatDate(iso)
const MAX_BALANCES = 3

/** One row's model: what the template shows, computed once per row. */
const items = computed(() =>
  expansion.lines(props.rows).map(({ node, depth, share }) => ({
    node,
    depth,
    nested: depth > 0,
    foldable: node.children.length > 0,
    open: expansion.isOpen(node.store.id),
    store: node.store,
    actions: actions(node),
    pinned: pinned(node.store),
    subtitle: walletSubtitle(node.store, t),
    balances: node.balances.slice(0, MAX_BALANCES),
    more: Math.max(0, node.balances.length - MAX_BALANCES),
    last: share !== null ? formatPct(share * 100, { decimals: 0 }) : node.last_tx_at ? lastDay(node.last_tx_at) : '—',
    sum: props.shown(Number(node.subtree_base)),
  })),
)
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-border bg-card shadow-sm" data-tour="stores-list">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead class="w-9 px-3"><span class="sr-only">{{ t('stores.table.pinned') }}</span></TableHead>
          <TableHead :class="[TH, 'w-2/5']" :aria-sort="ariaSort('name')">
            <Button variant="ghost" size="sm" class="-ml-2 h-7 px-2 text-xs font-medium text-muted-foreground" @click="sort = 'name'">
              {{ t('stores.table.wallet') }}
              <ArrowDown v-if="sort === 'name'" class="size-3 rotate-180" />
            </Button>
          </TableHead>
          <TableHead :class="TH">{{ t('stores.table.balance') }}</TableHead>
          <TableHead :class="TH">{{ t('stores.table.lastTx') }}</TableHead>
          <TableHead class="px-3 text-right" :aria-sort="ariaSort('value')">
            <Button variant="ghost" size="sm" class="-mr-2 h-7 px-2 text-xs font-medium text-muted-foreground" @click="sort = 'value'">
              <ArrowDown v-if="sort === 'value'" class="size-3" />
              {{ t('stores.table.inCurrency', { sign: currencySymbol(props.currency) }) }}
            </Button>
          </TableHead>
          <TableHead class="w-10 px-3"><span class="sr-only">{{ t('bonuses.actions') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <ContextMenu v-for="row in items" :key="row.store.id" :actions="row.actions">
          <TableRow :class="cn('relative', row.store.archived && 'opacity-60')" :data-store="row.store.id">
            <TableCell class="px-3 py-2.5">
              <Button
                v-if="!row.nested"
                variant="ghost"
                size="icon-sm"
                :class="cn('relative z-10', row.pinned ? 'text-warning' : 'text-border hover:text-muted-foreground')"
                :aria-label="row.pinned ? t('stores.unpin') : t('stores.pin')"
                :aria-pressed="row.pinned"
                @click="togglePin(row.store)"
              >
                <Star class="size-4" :fill="row.pinned ? 'currentColor' : 'none'" />
              </Button>
            </TableCell>
            <TableCell class="max-w-0 px-3 py-2.5">
              <div class="flex items-center gap-3" :style="row.nested ? { paddingInlineStart: `${row.depth * 1.5}rem` } : undefined">
                <span
                  :class="
                    cn(
                      'flex shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground',
                      row.nested ? 'size-7' : 'size-8',
                    )
                  "
                >
                  <component :is="storeIcon(row.store)" class="size-4" />
                </span>
                <span class="min-w-0">
                  <!-- The stretched link: its ::after covers the row, so a click anywhere opens the wallet. -->
                  <RouterLink
                    :to="{ name: 'wallet', params: { id: row.store.id } }"
                    :data-entity="`store:${row.store.id}`"
                    :class="LINK_CLASS"
                  >
                    {{ row.store.name }}
                  </RouterLink>
                  <span v-if="!row.nested" class="block truncate text-xs text-muted-foreground">{{ row.subtitle }}</span>
                </span>
                <WalletExpand v-if="row.foldable" :name="row.store.name" :open="row.open" @toggle="expansion.toggle(row.store.id)" />
              </div>
            </TableCell>
            <TableCell class="px-3 py-2.5 whitespace-nowrap tabular-nums">
              <span v-if="!row.balances.length" class="text-muted-foreground">—</span>
              <span v-for="balance in row.balances" :key="balance.asset.symbol" class="block">
                <Money :amount="balance.quantity" :currency="balance.asset.symbol" />
              </span>
              <span v-if="row.more" class="block text-xs text-muted-foreground">{{ t('stores.table.more', { count: row.more }) }}</span>
            </TableCell>
            <TableCell class="px-3 py-2.5 whitespace-nowrap text-muted-foreground">{{ row.last }}</TableCell>
            <TableCell class="px-3 py-2.5 text-right font-semibold whitespace-nowrap tabular-nums">
              <Money v-if="row.sum !== null" :amount="row.sum" :currency="props.currency" />
              <span v-else class="font-normal text-muted-foreground" :title="t('stores.table.noRate')">—</span>
            </TableCell>
            <TableCell class="px-3 py-2.5">
              <ActionMenu v-if="!row.nested" class="relative z-10" :actions="row.actions" :label="t('stores.actions', { name: row.store.name })" />
            </TableCell>
          </TableRow>
        </ContextMenu>
      </TableBody>
    </Table>
  </div>
</template>
