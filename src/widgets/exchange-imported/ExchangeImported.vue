<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { PendingImports } from '@/features/pending-imports'
import { TransactionRow } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'
import { openTxDetails } from '@/shared/lib/tx-details'

/**
 * The operations of one exchange wallet: what waits for confirmation, then
 * what was already brought in (marked «Импорт с биржи»), newest first.
 */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()

const PAGE = 50
const shown = ref(PAGE)
const imported = useQuery({
  queryKey: computed(() => ['imported-transactions', props.storeId, shown.value]),
  queryFn: () =>
    call<ImportedTransactions>('list_imported_transactions', {
      input: { store_id: props.storeId, limit: shown.value, offset: 0 },
    }),
})
const items = computed(() => imported.data.value?.items ?? [])
const total = computed(() => imported.data.value?.total ?? 0)
</script>

<template>
  <div class="grid gap-4" data-tour="exchange-imported">
    <section class="grid gap-2">
      <h3 class="text-sm font-semibold">{{ t('importedOps.pending') }}</h3>
      <PendingImports :store-id="props.storeId" compact />
    </section>
    <section class="grid gap-2">
      <h3 class="text-sm font-semibold">{{ t('importedOps.booked', { count: total }) }}</h3>
      <EmptyState v-if="imported.isSuccess.value && !items.length" compact :title="t('importedOps.empty')" />
      <div v-else class="grid divide-y divide-border rounded-md border border-border">
        <TransactionRow v-for="tx in items" :key="txId(tx)" :tx="tx" class="px-3 py-2" openable @open="openTxDetails" />
      </div>
      <Button v-if="items.length < total" variant="outline" size="sm" class="justify-self-start" @click="shown += PAGE">
        {{ t('importedOps.more', { count: total - items.length }) }}
      </Button>
    </section>
  </div>
</template>
