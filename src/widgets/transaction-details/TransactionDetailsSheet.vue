<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CopyPlus } from 'lucide-vue-next'

import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { Button } from '@/shared/ui/button'
import { EntityLink } from '@/shared/ui/entity-link'
import { Sheet, SheetContent } from '@/shared/ui/sheet'
import { call } from '@/shared/api/ipc'
import { txId, txMeta, txStoreId } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { useSimilarTxs } from './model/use-similar-txs'
import { useTxNeighbors } from './model/use-tx-neighbors'
import type { TransactionDetailsSheetProps } from './transaction-details.types'
import TxCardHead from './ui/TxCardHead.vue'
import TxFacts from './ui/TxFacts.vue'
import TxMore from './ui/TxMore.vue'
import TxSimilar from './ui/TxSimilar.vue'

/**
 * The operation's card (Paths 1 · 6): every value leads further, the rate is
 * checked against past exchanges, ‹ › step through the list it came from.
 */
const props = defineProps<TransactionDetailsSheetProps>()
const open = defineModel<boolean>('open', { required: true })

defineSlots<{
  /** A wallet or contact side, e.g. a wallet glance; defaults to its name. */
  side?(props: { side: AccountRef; label: string }): unknown
  relations?(): unknown
}>()

const { t } = useI18n()
const context = useTxContext()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
  enabled: computed(() => open.value),
})

const neighbours = useTxNeighbors(() => props.tx, open)
const similar = useSimilarTxs(() => props.tx, open)

const meta = computed(() => (props.tx ? txMeta(props.tx) : null))
const duplicateOpen = ref(false)
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent side="right" class="flex w-full flex-col gap-0 p-0 sm:max-w-md" data-tx-card>
      <template v-if="props.tx && meta">
        <div class="grid flex-1 content-start gap-5 overflow-y-auto pt-2 pb-6">
          <TxCardHead
            :tx="props.tx"
            :prev="Boolean(neighbours.prev.value)"
            :next="Boolean(neighbours.next.value)"
            :neighbours="neighbours.shown.value"
            @go="neighbours.go"
          />

          <div class="grid gap-5 px-4">
            <TxFacts
              :tx="props.tx"
              :delta="similar.delta.value"
              :fx-per-usd="summary.data.value?.fx_per_usd"
              :base-currency="summary.data.value?.base_currency"
            >
              <template #side="slotProps"><slot name="side" v-bind="slotProps">{{ slotProps.label }}</slot></template>
            </TxFacts>

            <div v-if="meta.categories.length || meta.comment" class="grid gap-1.5 text-sm">
              <div v-if="meta.categories.length" class="flex flex-wrap gap-x-3 gap-y-1">
                <EntityLink v-for="key in meta.categories" :key="key" :target="{ kind: 'category', id: key }" :label="context.categoryName(key)" />
              </div>
              <p v-if="meta.comment" class="text-muted-foreground">{{ meta.comment }}</p>
            </div>

            <TxSimilar v-if="similar.filter.value" :tx="props.tx" :items="similar.items.value" :filter="similar.filter.value" />

            <TxMore :tx="props.tx" :open="open">
              <slot name="relations" />
            </TxMore>
          </div>
        </div>

        <footer class="grid grid-cols-3 items-center gap-2 border-t sm:flex px-4 pt-3 pb-[max(0.75rem,var(--safe-bottom,0px))]" data-tx-card-actions>
          <TransactionDelete :id="txId(props.tx)" labelled />
          <Button variant="outline" class="sm:ms-auto" data-tx-repeat @click="duplicateOpen = true">
            <CopyPlus class="size-4 shrink-0" />
            {{ t('txCard.repeat') }}
          </Button>
          <TransactionEditButton :tx="props.tx" labelled />
        </footer>

        <TransactionCreateDialog
          v-if="duplicateOpen"
          v-model:open="duplicateOpen"
          :store-id="txStoreId(props.tx)"
          :prefill="props.tx"
        />
      </template>
    </SheetContent>
  </Sheet>
</template>
