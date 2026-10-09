<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TransactionsList } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { call } from '@/shared/api/ipc'
import { TX_KINDS, txKind, txSearchText } from '@/shared/lib/transactions'

import type { StoreTransactionsProps } from './store-transactions.types'

const props = withDefaults(defineProps<StoreTransactionsProps>(), { includeChildren: true })

/** Per-row actions (edit, delete) come from the page. */
defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()

const kindFilter = ref<'' | TxKind>('')
const search = ref('')

const txs = useQuery({
  queryKey: computed(() => ['transactions', props.storeId, props.includeChildren]),
  queryFn: () =>
    call<Transaction[]>('list_transactions', { storeId: props.storeId, includeChildren: props.includeChildren }),
})

const rows = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return (txs.data.value ?? []).filter((tx) => {
    if (kindFilter.value && txKind(tx) !== kindFilter.value) return false
    return !needle || txSearchText(tx).includes(needle)
  })
})
</script>

<template>
  <Card>
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-medium">{{ t('hist.title') }}</p>
        <div class="flex flex-wrap items-center gap-2">
          <Select v-model="kindFilter" class="h-8 w-40 text-xs" :aria-label="t('form.kind')">
            <option value="">{{ t('hist.allKinds') }}</option>
            <option v-for="kind in TX_KINDS" :key="kind" :value="kind">{{ t(`tx.kind.${kind}`) }}</option>
          </Select>
          <Input v-model="search" class="h-8 w-48 text-xs" :placeholder="t('hist.search')" />
        </div>
      </div>
    </template>
    <div class="-my-3">
      <TransactionsList :transactions="rows" :store-id="props.storeId">
        <template #actions="{ tx }">
          <slot name="actions" :tx="tx" />
        </template>
      </TransactionsList>
    </div>
  </Card>
</template>
