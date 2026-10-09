<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Check, ExternalLink, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { call, parseError } from '@/shared/api/ipc'
import {
  DEFAULT_TX_BADGE,
  TX_KIND_BADGE,
  TX_TAGS,
  type TxTag,
  amountClass,
  txChainAndHash,
  txDetails,
  txFee,
  txId,
  txLegForStore,
  txMeta,
  txSearchText,
  txTag,
} from '@/shared/lib/transactions'
import { shortHash } from '@/shared/lib/utils'

import type { StoreHistoryProps } from './store-history.types'

const props = withDefaults(defineProps<StoreHistoryProps>(), { includeChildren: true })

/** Per-row actions (delete…) come from the page: entities do not own user actions. */
defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const kindFilter = ref<'' | TxTag>('')
const search = ref('')
const editingId = ref<string | null>(null)
const editingText = ref('')

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const txs = useQuery({
  queryKey: computed(() => ['transactions', props.storeId, props.includeChildren]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', { storeId: props.storeId, includeChildren: props.includeChildren }),
})

const storeName = (id: string) => stores.data.value?.find((tx) => tx.id === id)?.name ?? id

/** This store's side of the transaction: a trade or transfer has two. */
const legOf = (tx: Transaction) => txLegForStore(tx, props.storeId)

/** Counterparty of a transfer, for a quick "Binance → Ledger" read. */
const counterparty = (tx: Transaction): string => {
  const leg = legOf(tx)
  return leg.to_store_id ? storeName(leg.to_store_id) : ''
}

const rows = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return (txs.data.value ?? []).filter((tx) => {
    if (kindFilter.value && txTag(tx) !== kindFilter.value) return false
    return !needle || txSearchText(tx).includes(needle)
  })
})

const invalidations = () => {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  // A parent's overview rolls this store up too, so refresh every store view.
  void queryClient.invalidateQueries({ queryKey: ['store'] })
  void queryClient.invalidateQueries({ queryKey: ['store-history'] })
  void queryClient.invalidateQueries({ queryKey: ['store-pnl'] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
  void queryClient.invalidateQueries({ queryKey: ['history'] })
}

const saveComment = useMutation({
  mutationFn: () =>
    call('update_comment', {
      input: { transaction_id: editingId.value, comment: editingText.value.trim() },
    }),
  onSuccess: () => {
    editingId.value = null
    editingText.value = ''
    toast.success(t('tx.commentSaved'))
    invalidations()
  },
  onError: (err) => toast.error(t('tx.commentSaveFailed'), { description: parseError(err).message }),
})

function startEdit(tx: Transaction) {
  editingId.value = txId(tx)
  editingText.value = txMeta(tx).comment
}

const hashOf = (tx: Transaction) => txChainAndHash(tx).hash

async function openExplorer(tx: Transaction) {
  const url = explorerUrl(tx)
  if (url) await call('open_explorer', { url })
}

async function copyHash(tx: Transaction) {
  const hash = hashOf(tx)
  if (hash) await navigator.clipboard?.writeText(hash)
}

/** Mirrors `Transaction::explorer_url` for the networks we surface in the UI. */
function explorerUrl(tx: Transaction): string | null {
  const { chain, hash } = txChainAndHash(tx)
  if (!hash) return null
  const map: Partial<Record<Chain, string>> = {
    bitcoin: `https://mempool.space/tx/${hash}`,
    ethereum: `https://etherscan.io/tx/${hash}`,
    bsc: `https://bscscan.com/tx/${hash}`,
    polygon: `https://polygonscan.com/tx/${hash}`,
    arbitrum: `https://arbiscan.io/tx/${hash}`,
    optimism: `https://optimistic.etherscan.io/tx/${hash}`,
    base: `https://basescan.org/tx/${hash}`,
    avalanche: `https://snowtrace.io/tx/${hash}`,
    solana: `https://solscan.io/tx/${hash}`,
    tron: `https://tronscan.org/#/transaction/${hash}`,
  }
  return map[chain ?? 'fiat'] ?? null
}
</script>

<template>
  <Card>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-medium">{{ t('hist.title') }}</p>
        <div class="flex flex-wrap items-center gap-2">
          <Select v-model="kindFilter" class="h-8 w-40 text-xs">
            <option value="">{{ t('hist.allKinds') }}</option>
            <option v-for="kind in TX_TAGS" :key="kind" :value="kind">{{ t(`tx.kind.${kind}`) }}</option>
          </Select>
          <Input v-model="search" class="h-8 w-48 text-xs" :placeholder="t('hist.search')" />
        </div>
      </div>
    </template>

    <div class="divide-y divide-border">
      <div v-for="tx in rows" :key="txId(tx)" class="py-3 first:pt-0">
        <div class="flex items-start gap-3">
          <Badge :variant="TX_KIND_BADGE[txTag(tx)] ?? DEFAULT_TX_BADGE" class="mt-0.5 shrink-0">
            {{ t(`tx.kind.${txTag(tx)}`) }}
          </Badge>

          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
              <span class="font-medium">{{ legOf(tx).asset.symbol.toUpperCase() }}</span>
              <span class="truncate text-sm text-muted-foreground">{{ legOf(tx).asset.name }}</span>
              <Badge
                v-for="entry in txMeta(tx).categories"
                :key="entry"
                variant="outline"
                class="shrink-0"
              >
                {{ entry }}
              </Badge>
              <!-- Booked on a sub-account of this wallet: say which one. -->
              <Badge v-if="legOf(tx).store_id !== props.storeId" variant="secondary" class="shrink-0">
                {{ storeName(legOf(tx).store_id) }}
              </Badge>
              <span v-if="counterparty(tx)" class="truncate text-xs text-muted-foreground">
                {{ legOf(tx).amount.startsWith('-') ? '→' : '←' }} {{ counterparty(tx) }}
              </span>
            </div>

            <p class="text-xs text-muted-foreground">
              {{ formatDateTime(txMeta(tx).timestamp) }}
              <span v-if="txFee(tx)"> · {{ t('hist.fee') }} {{ txFee(tx)?.amount }} {{ txFee(tx)?.asset.toUpperCase() }}</span>
            </p>
            <!-- The facts only this kind has: rail, counterparty, pool… -->
            <p v-if="txDetails(tx).length" class="truncate text-xs text-muted-foreground">
              {{ txDetails(tx).map(([key, value]) => `${t(key)}: ${value}`).join(' · ') }}
            </p>

            <div v-if="editingId === txId(tx)" class="mt-1.5 flex items-center gap-2">
              <Input v-model="editingText" class="h-8" :placeholder="t('tx.comment')" @keyup.enter="saveComment.mutate()" />
              <Button variant="ghost" size="icon" :title="t('actions.save')" @click="saveComment.mutate()">
                <Check class="size-4" />
              </Button>
              <Button variant="ghost" size="icon" :title="t('actions.cancel')" @click="editingId = null">
                <X class="size-4" />
              </Button>
            </div>
            <button
              v-else-if="txMeta(tx).comment"
              type="button"
              class="block max-w-full truncate text-left text-xs text-muted-foreground hover:text-foreground"
              :title="t('hist.editComment')"
              @click="startEdit(tx)"
            >
              {{ txMeta(tx).comment }}
            </button>
          </div>

          <div class="shrink-0 text-right">
            <p class="whitespace-nowrap font-medium tabular-nums" :class="amountClass(legOf(tx).amount)">
              {{ legOf(tx).amount }} {{ legOf(tx).asset.symbol.toUpperCase() }}
            </p>
            <p v-if="legOf(tx).price_usd" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
              {{ formatMoney(legOf(tx).price_usd ?? '', { currency: 'usd' }) }}
            </p>
          </div>

          <div class="flex shrink-0 items-center gap-0.5">
            <Button v-if="hashOf(tx)" variant="ghost" size="icon" :title="t('hist.openExplorer')" @click="openExplorer(tx)">
              <ExternalLink class="size-4" />
            </Button>
            <Button
              v-if="hashOf(tx)"
              variant="ghost"
              size="icon"
              :title="t('hist.copyHash')"
              @click="copyHash(tx)"
            >
              <span class="text-xs tabular-nums">{{ shortHash(hashOf(tx) ?? '', 6) }}</span>
            </Button>
            <slot name="actions" :tx="tx" />
          </div>
        </div>
      </div>

      <p v-if="!rows.length" class="py-6 text-center text-sm text-muted-foreground">
        {{ txs.data.value?.length ? t('hist.noMatches') : t('hist.empty') }}
      </p>
    </div>
  </Card>
</template>
