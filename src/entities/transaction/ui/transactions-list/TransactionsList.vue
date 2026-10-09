<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { txId, txStoreId } from '@/shared/lib/transactions'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import type { TransactionsListProps } from './transactions-list.types'

const props = defineProps<TransactionsListProps>()

defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('tx.title') }}</p>
    </template>

    <div class="divide-y divide-border">
      <TransactionRow
        v-for="tx in props.transactions"
        :key="txId(tx)"
        :tx="tx"
        :store-name="props.storeName(txStoreId(tx))"
      >
        <template #actions="{ tx: row }">
          <slot name="actions" :tx="row" />
        </template>
      </TransactionRow>

      <p v-if="!props.transactions.length" class="py-6 text-center text-sm text-muted-foreground">
        {{ t('tx.empty') }}
      </p>
    </div>
  </Card>
</template>
