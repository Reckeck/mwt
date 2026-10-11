<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueries, useQuery } from '@tanstack/vue-query'
import { Receipt } from 'lucide-vue-next'

import { UtilityBillDialog } from '@/features/utility-bill'
import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

/** This month's utilities per property: what is due, when, and one button to pay it all. */
const { t } = useI18n()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const items = useQuery({ queryKey: ['utilities'], queryFn: () => call<UtilityItem[]>('list_utility_items') })
const properties = computed(() => {
  const withBills = new Set((items.data.value ?? []).filter((item) => item.active).map((item) => item.property_id))
  return flattenStoreTree(tree.data.value ?? [])
    .map((node) => node.store)
    .filter((store) => store.kind === 'property' && !store.archived && withBills.has(store.id))
})
const bills = useQueries({
  queries: computed(() =>
    properties.value.map((store) => ({
      queryKey: ['utility-bill', store.id, 'current'],
      queryFn: () => call<UtilityBill>('get_utility_bill', { propertyId: store.id, period: null }),
    })),
  ),
})

const rows = computed(() =>
  properties.value.map((store, index) => {
    const bill = bills.value[index]?.data
    const due = new Map<string, number>()
    let open = 0
    for (const line of bill?.lines ?? []) {
      if (Number(line.paid) > 0) continue
      open += 1
      due.set(line.item.currency, (due.get(line.item.currency) ?? 0) + Number(line.amount ?? 0))
    }
    return { store, bill, open, due: [...due.entries()], waiting: bill?.lines.some((line) => line.amount === null) ?? false }
  }),
)

const payFor = ref('')
const payOpen = computed({
  get: () => payFor.value !== '',
  set: (value: boolean) => {
    if (!value) payFor.value = ''
  },
})
</script>

<template>
  <Card size="sm">
    <template #header>
      <p data-widget-title class="text-sm text-muted-foreground">{{ t('utilitiesDue.title') }}</p>
    </template>
    <Skeleton v-if="tree.isPending.value || items.isPending.value" class="h-20 w-full" />
    <p v-else-if="!rows.length" class="text-sm text-muted-foreground">{{ t('utilities.noProperty') }}</p>
    <ul v-else class="grid gap-3">
      <li v-for="row in rows" :key="row.store.id" class="grid gap-1.5">
        <div class="flex items-center justify-between gap-2">
          <span class="truncate text-sm font-medium">{{ row.store.name }}</span>
          <span v-if="row.bill" class="shrink-0 text-xs text-muted-foreground">
            {{ t('utilitiesDue.due', { date: formatDate(row.bill.due_at) }) }}
          </span>
        </div>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <span v-if="row.open" class="flex flex-wrap items-baseline gap-2 text-lg font-semibold">
            <Money v-for="[currency, sum] in row.due" :key="currency" :amount="sum" :currency="currency" :decimals="0" />
          </span>
          <span v-else class="text-sm text-emerald-600 dark:text-emerald-400">{{ t('utilitiesDue.allPaid') }}</span>
          <Button v-if="row.open" size="sm" @click="payFor = row.store.id">
            <Receipt class="size-4" />
            {{ t('utilityBill.open') }}
          </Button>
        </div>
        <p v-if="row.waiting" class="text-xs text-muted-foreground">{{ t('utilitiesDue.waiting') }}</p>
      </li>
    </ul>
    <UtilityBillDialog v-if="payFor" v-model:open="payOpen" :property-id="payFor" />
  </Card>
</template>
