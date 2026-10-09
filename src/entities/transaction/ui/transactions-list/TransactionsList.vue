<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { txId } from '@/shared/lib/transactions'
import TransactionRow from '../transaction-row/TransactionRow.vue'

import type { TransactionsListProps } from './transactions-list.types'

const props = withDefaults(defineProps<TransactionsListProps>(), { storeId: null })

defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
</script>

<template>
  <div class="divide-y divide-border">
    <TransactionRow v-for="tx in props.transactions" :key="txId(tx)" :tx="tx" :store-id="props.storeId">
      <template #actions="{ tx: row }">
        <slot name="actions" :tx="row" />
      </template>
    </TransactionRow>

    <p v-if="!props.transactions.length" class="py-6 text-center text-sm text-muted-foreground">
      {{ t('tx.empty') }}
    </p>
  </div>
</template>
