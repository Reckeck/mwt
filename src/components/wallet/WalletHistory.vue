<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Check, ExternalLink, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Input from '@/components/ui/input.vue'
import Select from '@/components/ui/select.vue'
import { formatDateTime, formatMoney } from '@/lib/format'
import { call, parseError } from '@/lib/ipc'
import { cn, shortHash } from '@/lib/utils'

const props = defineProps<{ storeId: string }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const kindFilter = ref<'' | TxKind>('')
const search = ref('')
const editingId = ref<string | null>(null)
const editingText = ref('')

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const txs = useQuery({
  queryKey: ['transactions', props.storeId],
  queryFn: () => call<Transaction[]>('list_transactions', { storeId: props.storeId }),
})

const kinds: TxKind[] = ['income', 'expense', 'payment', 'transfer', 'trade', 'fee', 'paper_trade']

const kindBadge: Record<TxKind, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  income: 'default',
  expense: 'destructive',
  payment: 'destructive',
  transfer: 'secondary',
  trade: 'secondary',
  fee: 'outline',
  paper_trade: 'outline',
}

const storeName = (id: string) => stores.data.value?.find((tx) => tx.id === id)?.name ?? id

/** Counterparty of a transfer leg, for a quick "Binance → Ledger" read. */
const counterparty = (tx: Transaction): string => {
  if (!tx.to_store_id) return ''
  if (tx.store_id === props.storeId) return storeName(tx.to_store_id)
  return storeName(tx.store_id)
}

const rows = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return (txs.data.value ?? []).filter((tx) => {
    if (kindFilter.value && tx.kind !== kindFilter.value) return false
    if (!needle) return true
    return (
      tx.asset.symbol.toLowerCase().includes(needle) ||
      tx.comment.toLowerCase().includes(needle) ||
      tx.categories.some((entry) => entry.toLowerCase().includes(needle)) ||
      (tx.tx_hash ?? '').toLowerCase().includes(needle)
    )
  })
})

const invalidations = () => {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  void queryClient.invalidateQueries({ queryKey: ['store', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['store-history', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['store-pnl', props.storeId] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
  void queryClient.invalidateQueries({ queryKey: ['history'] })
}

const remove = useMutation({
  mutationFn: (id: string) => call('delete_transaction', { id }),
  onSuccess: () => {
    toast.success(t('tx.deleted'))
    invalidations()
  },
  onError: (err) => toast.error(t('tx.deleteFailed'), { description: parseError(err).message }),
})

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
  editingId.value = tx.id
  editingText.value = tx.comment
}

async function openExplorer(tx: Transaction) {
  if (!tx.tx_hash) return
  const url = explorerUrl(tx)
  if (url) await call('open_explorer', { url })
}

async function copyHash(tx: Transaction) {
  if (tx.tx_hash) await navigator.clipboard?.writeText(tx.tx_hash)
}

/** Mirrors `Transaction::explorer_url` for the networks we surface in the UI. */
function explorerUrl(tx: Transaction): string | null {
  const hash = tx.tx_hash
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
  return map[tx.network ?? 'fiat'] ?? null
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
            <option v-for="kind in kinds" :key="kind" :value="kind">{{ t(`tx.kind.${kind}`) }}</option>
          </Select>
          <Input v-model="search" class="h-8 w-48 text-xs" :placeholder="t('hist.search')" />
        </div>
      </div>
    </template>

    <div class="divide-y divide-border">
      <div v-for="tx in rows" :key="tx.id" class="py-3 first:pt-0">
        <div class="flex items-start gap-3">
          <Badge :variant="kindBadge[tx.kind] ?? 'secondary'" class="mt-0.5 shrink-0">
            {{ t(`tx.kind.${tx.kind}`) }}
          </Badge>

          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
              <span class="font-medium">{{ tx.asset.symbol.toUpperCase() }}</span>
              <span class="truncate text-sm text-muted-foreground">{{ tx.asset.name }}</span>
              <Badge
                v-for="entry in tx.categories"
                :key="entry"
                variant="outline"
                class="shrink-0"
              >
                {{ entry }}
              </Badge>
              <span v-if="counterparty(tx)" class="truncate text-xs text-muted-foreground">
                {{ tx.store_id === storeId ? '→' : '←' }} {{ counterparty(tx) }}
              </span>
            </div>

            <p class="text-xs text-muted-foreground">
              {{ formatDateTime(tx.timestamp) }}
              <span v-if="tx.fee"> · {{ t('hist.fee') }} {{ tx.fee }} {{ tx.fee_asset ?? tx.asset.symbol.toUpperCase() }}</span>
            </p>

            <div v-if="editingId === tx.id" class="mt-1.5 flex items-center gap-2">
              <Input v-model="editingText" class="h-8" :placeholder="t('tx.comment')" @keyup.enter="saveComment.mutate()" />
              <Button variant="ghost" size="icon" :title="t('actions.save')" @click="saveComment.mutate()">
                <Check class="size-4" />
              </Button>
              <Button variant="ghost" size="icon" :title="t('actions.cancel')" @click="editingId = null">
                <X class="size-4" />
              </Button>
            </div>
            <button
              v-else-if="tx.comment"
              type="button"
              class="block max-w-full truncate text-left text-xs text-muted-foreground hover:text-foreground"
              :title="t('hist.editComment')"
              @click="startEdit(tx)"
            >
              {{ tx.comment }}
            </button>
          </div>

          <div class="shrink-0 text-right">
            <p
              class="whitespace-nowrap font-medium tabular-nums"
              :class="
                cn(
                  tx.kind === 'income' && 'text-emerald-600 dark:text-emerald-400',
                  (tx.kind === 'expense' || tx.kind === 'payment') && 'text-destructive',
                )
              "
            >
              {{ tx.amount }} {{ tx.asset.symbol.toUpperCase() }}
            </p>
            <p v-if="tx.price_usd" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
              {{ formatMoney(tx.price_usd, { currency: 'usd' }) }}
            </p>
          </div>

          <div class="flex shrink-0 items-center gap-0.5">
            <Button v-if="tx.tx_hash" variant="ghost" size="icon" :title="t('hist.openExplorer')" @click="openExplorer(tx)">
              <ExternalLink class="size-4" />
            </Button>
            <Button
              v-if="tx.tx_hash"
              variant="ghost"
              size="icon"
              :title="t('hist.copyHash')"
              @click="copyHash(tx)"
            >
              <span class="text-xs tabular-nums">{{ shortHash(tx.tx_hash, 6) }}</span>
            </Button>
            <Button variant="ghost" size="icon" :title="t('tx.delete')" @click="remove.mutate(tx.id)">
              <X class="size-4 text-destructive" />
            </Button>
          </div>
        </div>
      </div>

      <p v-if="!rows.length" class="py-6 text-center text-sm text-muted-foreground">
        {{ txs.data.value?.length ? t('hist.noMatches') : t('hist.empty') }}
      </p>
    </div>
  </Card>
</template>
