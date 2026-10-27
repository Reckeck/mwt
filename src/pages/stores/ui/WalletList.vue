<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import { Money } from '@/entities/currency'
import { formatPct, pluralForm } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { walletSubtitle } from '@/entities/store'
import { useWalletExpansion } from '../model/use-wallet-expansion'
import WalletExpand from './WalletExpand.vue'

/**
 * The wallets on a phone (Currency-1 · 2): the name and kind on the left; on the
 * right what it holds in its own currency (or «2 валюты») and ≈ the sum. A wallet
 * with sub-accounts has a chevron; unfolded, they follow indented, under the name
 * «RUB · 55 %».
 */
const props = defineProps<{ rows: StoreTreeNode[]; currency: string; shown: (amount: number) => number | null }>()
const { t, locale } = useI18n()
const expansion = useWalletExpansion()

const lines = computed(() =>
  expansion.lines(props.rows).map(({ node, depth, share }) => ({
    node,
    nested: depth > 0,
    indent: depth * 1.25,
    currencies:
      node.balances.length > 1
        ? t(`stores.table.currencies.${pluralForm(node.balances.length, locale.value)}`, { count: node.balances.length })
        : '',
    converted: props.shown(Number(node.subtree_base)),
    foldable: node.children.length > 0,
    open: expansion.isOpen(node.store.id),
    subtitle:
      share === null
        ? walletSubtitle(node.store, t)
        : t('stores.table.subShare', {
            currency: (node.balances[0]?.asset.symbol ?? props.currency).toUpperCase(),
            share: formatPct(share * 100, { decimals: 0 }),
          }),
  })),
)
</script>

<template>
  <div class="divide-y rounded-xl border border-border bg-card px-3 shadow-sm" data-tour="stores-list">
    <div
      v-for="line in lines"
      :key="line.node.store.id"
      :class="cn('relative flex min-h-13 items-center gap-3 py-2.5', line.node.store.archived && 'opacity-60')"
      :style="line.nested ? { paddingInlineStart: `${line.indent}rem` } : undefined"
      :data-store="line.node.store.id"
    >
      <span
        :class="
          cn(
            'flex shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground',
            line.nested ? 'size-8' : 'size-9',
          )
        "
      >
        <component :is="storeIcon(line.node.store)" class="size-4" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <!-- The stretched link: its ::after covers the row; the chevron sits above it. -->
        <RouterLink
          :to="{ name: 'wallet', params: { id: line.node.store.id } }"
          :data-entity="`store:${line.node.store.id}`"
          class="truncate text-sm font-medium after:absolute after:inset-0"
        >
          {{ line.node.store.name }}
        </RouterLink>
        <span class="truncate text-xs text-muted-foreground">{{ line.subtitle }}</span>
      </span>
      <WalletExpand v-if="line.foldable" :name="line.node.store.name" :open="line.open" @toggle="expansion.toggle(line.node.store.id)" />
      <span class="flex shrink-0 flex-col items-end">
        <span class="text-sm font-semibold whitespace-nowrap tabular-nums">
          <template v-if="line.currencies">{{ line.currencies }}</template>
          <Money v-else-if="line.node.balances.length" :amount="line.node.balances[0].quantity" :currency="line.node.balances[0].asset.symbol" />
          <Money v-else :amount="0" :currency="props.currency" />
        </span>
        <span v-if="line.converted !== null && line.node.balances.length" class="text-xs whitespace-nowrap text-muted-foreground">
          ≈ <Money :amount="line.converted" :currency="props.currency" />
        </span>
      </span>
    </div>
  </div>
</template>
