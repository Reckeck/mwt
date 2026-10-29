<script setup lang="ts">
import { computed } from 'vue'

import { useTxPages } from '@/entities/transaction'
import { Money } from '@/entities/currency'
import { useStoreTree } from '@/shared/api/queries'
import { formatDayMonthShort } from '@/shared/lib/format'
import { accountStoreId, txMeta } from '@/shared/lib/transactions'
import { storeTxFilter } from '@/shared/lib/tx-filter'

/**
 * `Received from Bybit Oct 17 · 0.04 BTC, network fee 0.0002 BTC` under the assets of a crypto
 * wallet (Wallets-1 · 6): the last transfer that came in from another of the user's wallets.
 */
const props = defineProps<{ storeId: string }>()
const tree = useStoreTree()
const filter = computed(() => storeTxFilter(props.storeId, false))
const { rows } = useTxPages(filter)

/** The top-level wallet the sender sits in, so a sub-account reads as its exchange. */
function topName(id: string): string | null {
  for (const root of tree.data.value ?? []) {
    const stack = [root]
    while (stack.length) {
      const node = stack.pop()
      if (!node) break
      if (node.store.id === id) return root.store.name
      stack.push(...node.children)
    }
  }
  return null
}

const incoming = computed(() => {
  for (const tx of rows.value) {
    if (!('transfer' in tx)) continue
    const { from, to, asset, amount, fee } = tx.transfer
    const sender = accountStoreId(from)
    if (accountStoreId(to) !== props.storeId || !sender || sender === props.storeId) continue
    const name = topName(sender)
    if (!name) continue
    return { name, date: formatDayMonthShort(txMeta(tx).timestamp), asset: asset.symbol, amount, fee }
  }
  return null
})
</script>

<template>
  <p v-if="incoming" class="text-xs text-muted-foreground" data-wallet-incoming>
    <i18n-t :keypath="incoming.fee ? 'wallet.incomingFee' : 'wallet.incoming'">
      <template #from>{{ incoming.name }}</template>
      <template #date>{{ incoming.date }}</template>
      <template #amount><Money :amount="incoming.amount" :currency="incoming.asset" /></template>
      <template #fee><Money v-if="incoming.fee" :amount="incoming.fee.amount" :currency="incoming.fee.asset.symbol" /></template>
    </i18n-t>
  </p>
</template>
