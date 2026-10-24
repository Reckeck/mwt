<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import { Money } from '@/entities/currency'
import { useStoreTree } from '@/shared/api/queries'
import { formatQuantity } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { flattenStoreTree } from '@/shared/lib/store-tree'

/**
 * An exchange by its sub-accounts (Wallets-1 · 7): its own spot and each
 * sub-account, the sum in the base and what it holds in coins under it.
 * A phone lists them as rows of one card, the sum on the right.
 */
const props = defineProps<{
  store: Store
  children: StoreChildRow[]
  ownBase: string
  ownPositions: StorePosition[]
  base: string
  decimals: number
}>()
const { t } = useI18n()
const tree = useStoreTree()
const phone = useIsPhone()

const coins = (list: { asset: AssetRef; quantity: Amount }[]) =>
  list
    .filter((row) => Number(row.quantity) !== 0)
    .map((row) => `${formatQuantity(row.quantity, props.decimals)} ${row.asset.symbol.toUpperCase()}`)
    .join(' · ')
/** A futures account: «маржа 400 USDT», the coin it keeps its margin in (Wallets-2 · 9). */
function line(store: Store, balances: AssetBalance[]) {
  const margin = store.margin?.symbol.toUpperCase()
  if (!margin) return coins(balances)
  const held = balances.find((row) => row.asset.symbol.toUpperCase() === margin)?.quantity ?? '0'
  return t('exchangeSub.marginShort', { amount: `${formatQuantity(held, props.decimals)} ${margin}` })
}

const cards = computed(() => {
  const nodes = flattenStoreTree(tree.data.value ?? [])
  return [
    { id: props.store.id, label: t('wallet.spotAccount'), sum: props.ownBase, line: coins(props.ownPositions), to: null },
    ...props.children.map((row) => ({
      id: row.store.id,
      label: row.store.name,
      sum: row.subtree_base,
      line: line(row.store, nodes.find((node) => node.store.id === row.store.id)?.balances ?? []),
      to: { name: 'wallet', params: { id: row.store.id } },
    })),
  ]
})
</script>

<template>
  <div v-if="phone" class="rounded-xl border border-border bg-card px-3 shadow-sm" data-tour="wallet-sub-accounts">
    <component
      :is="card.to ? RouterLink : 'div'"
      v-for="card in cards"
      :key="card.id"
      :to="card.to ?? undefined"
      class="flex min-h-13 items-center gap-3 border-b border-border py-1.5 last:border-0"
    >
      <span class="flex min-w-0 flex-1 flex-col">
        <span class="truncate font-medium">{{ card.label }}</span>
        <span v-if="card.line" class="truncate text-xs text-muted-foreground">{{ card.line }}</span>
      </span>
      <span class="font-medium tabular-nums"><Money :amount="card.sum" :currency="props.base" :decimals="2" /></span>
    </component>
  </div>
  <div v-else class="grid gap-4 @md:grid-cols-2" data-tour="wallet-sub-accounts">
    <component
      :is="card.to ? RouterLink : 'div'"
      v-for="card in cards"
      :key="card.id"
      :to="card.to ?? undefined"
      class="flex min-w-0 flex-col rounded-xl border border-border bg-card p-4 shadow-sm"
      :class="card.to && 'transition-colors hover:bg-accent/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'"
    >
      <span class="text-caption text-muted-foreground">{{ card.label }}</span>
      <span class="text-xl font-semibold tabular-nums"><Money :amount="card.sum" :currency="props.base" :decimals="2" /></span>
      <span v-if="card.line" class="truncate text-xs text-muted-foreground">{{ card.line }}</span>
    </component>
  </div>
</template>
