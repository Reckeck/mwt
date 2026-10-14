<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
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

/**
 * A long history renders a page at a time: rows differ in height (badges,
 * comments, fees), so a fixed-height virtual list would jump; «Показать ещё»
 * adds the next page. A new filter starts from the first page again.
 */
const PAGE = 100
const limit = ref(PAGE)
watch(
  () => props.transactions,
  () => {
    limit.value = PAGE
  },
)
const shown = computed(() => props.transactions.slice(0, limit.value))
const rest = computed(() => props.transactions.length - shown.value.length)
</script>

<template>
  <div class="divide-y divide-border">
    <ContextMenu v-for="tx in shown" :key="txId(tx)" :actions="props.rowActions?.(tx) ?? []">
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

    <div v-if="rest > 0" class="flex justify-center py-3">
      <Button variant="outline" size="sm" @click="limit += PAGE">
        {{ t('ui.showMore', { count: Math.min(rest, PAGE) }) }}
      </Button>
    </div>

    <p v-if="!props.transactions.length" class="py-6 text-center text-sm text-muted-foreground">
      {{ t('tx.empty') }}
    </p>
  </div>
</template>
