<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ContextMenu } from '@/shared/ui/context-menu'
import { txId } from '@/shared/lib/transactions'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import type { TransactionsListProps } from './transactions-list.types'

const props = withDefaults(defineProps<TransactionsListProps>(), {
  storeId: null,
  losses: null,
  openable: false,
  rowActions: null,
})
const emit = defineEmits<{ open: [tx: Transaction] }>()

defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
</script>

<template>
  <div class="divide-y divide-border">
    <ContextMenu v-for="tx in props.transactions" :key="txId(tx)" :actions="props.rowActions?.(tx) ?? []">
    <TransactionRow
      :tx="tx"
      :store-id="props.storeId"
      :loss="props.losses?.[txId(tx)] ?? null"
      :openable="props.openable"
      @open="(row) => emit('open', row)"
    >
      <template #actions="{ tx: row }">
        <slot name="actions" :tx="row" />
      </template>
    </TransactionRow>
    </ContextMenu>

    <p v-if="!props.transactions.length" class="py-6 text-center text-sm text-muted-foreground">
      {{ t('tx.empty') }}
    </p>
  </div>
</template>
