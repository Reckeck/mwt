<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import { Money } from '@/entities/currency'
import { pluralForm } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'
import { cn } from '@/shared/lib/utils'

import { walletSubtitle } from '@/entities/store'

/**
 * The wallets on a phone (Currency-1 · 2): the name and kind on the left; on the
 * right what it holds in its own currency (or «2 валюты») and ≈ the sum.
 */
const props = defineProps<{ rows: StoreTreeNode[]; currency: string; shown: (amount: number) => number | null }>()
const { t, locale } = useI18n()
</script>

<template>
  <div class="divide-y rounded-xl border border-border bg-card px-3 shadow-sm" data-tour="stores-list">
    <RouterLink
      v-for="node in props.rows"
      :key="node.store.id"
      :to="{ name: 'wallet', params: { id: node.store.id } }"
      :class="cn('flex min-h-13 items-center gap-3 py-2.5', node.store.archived && 'opacity-60')"
      :data-store="node.store.id"
    >
      <span class="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground">
        <component :is="storeIcon(node.store)" class="size-4" />
      </span>
      <span class="flex min-w-0 flex-1 flex-col">
        <span class="truncate text-sm font-medium">{{ node.store.name }}</span>
        <span class="truncate text-xs text-muted-foreground">{{ walletSubtitle(node.store, t) }}</span>
      </span>
      <span class="flex shrink-0 flex-col items-end">
        <span class="text-sm font-semibold whitespace-nowrap tabular-nums">
          <template v-if="node.balances.length > 1">{{ t(`stores.table.currencies.${pluralForm(node.balances.length, locale)}`, { count: node.balances.length }) }}</template>
          <Money v-else-if="node.balances.length" :amount="node.balances[0].quantity" :currency="node.balances[0].asset.symbol" />
          <Money v-else :amount="0" :currency="props.currency" />
        </span>
        <span v-if="node.balances.length && props.shown(Number(node.subtree_base)) !== null" class="text-xs whitespace-nowrap text-muted-foreground">
          ≈ <Money :amount="props.shown(Number(node.subtree_base)) ?? 0" :currency="props.currency" />
        </span>
      </span>
    </RouterLink>
  </div>
</template>
