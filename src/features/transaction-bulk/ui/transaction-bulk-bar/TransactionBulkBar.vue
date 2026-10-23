<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { Pencil, Tag, Trash2, Wallet, X } from 'lucide-vue-next'

import { formatMoney } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { txId } from '@/shared/lib/transactions'
import { ActionSheet } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import type { UiAction } from '@/shared/ui/context-menu'

import { bulkTotals, movable } from '../../lib/bulk-totals'
import { useTransactionBulk } from '../../model/use-transaction-bulk'
import { BulkPickDialog, type BulkPickKind } from '../bulk-pick-dialog'
import type { TransactionBulkBarProps } from './transaction-bulk-bar.types'

const props = defineProps<TransactionBulkBarProps>()

const { t } = useI18n()
const phone = useIsPhone()

const picked = computed(() => props.transactions.filter((tx) => props.selection.has(txId(tx))))
const totals = computed(() =>
  bulkTotals(picked.value)
    .map((total) => formatMoney(total.amount, { currency: total.symbol, signed: total.signed }))
    .join(' · '),
)
/** A transfer or an exchange has two wallets: it can't move to one. */
const cantMove = computed(() => (picked.value.every(movable) ? false : t('bulk.cantMove')))

const { remove, categorize, move } = useTransactionBulk(() => props.selection.clear())

const pickKind = ref<BulkPickKind>('category')
const pickOpen = ref(false)
const sheetOpen = ref(false)
function pick(kind: BulkPickKind) {
  pickKind.value = kind
  pickOpen.value = true
}
function apply(value: string, name: string) {
  const ids = [...props.selection.ids.value]
  const done = { onSuccess: () => (pickOpen.value = false) }
  if (pickKind.value === 'category') categorize.mutate({ ids, category: value, name }, done)
  else move.mutate({ ids, storeId: value, name }, done)
}

const sheetActions = computed<UiAction[]>(() => [
  { id: 'category', label: t('bulk.category'), icon: Tag, run: () => pick('category') },
  { id: 'store', label: t('bulk.wallet'), icon: Wallet, disabled: cantMove.value, run: () => pick('store') },
])

// Esc drops the pick, unless a dialog on top takes it first.
useEventListener(document, 'keydown', (event: KeyboardEvent) => {
  if (event.key !== 'Escape' || event.defaultPrevented || pickOpen.value || sheetOpen.value) return
  props.selection.clear()
})
</script>

<template>
  <div v-if="phone" class="grid gap-1" data-tx-bulk>
    <div class="flex h-11 items-center gap-1 rounded-[10px] bg-primary/12 px-1">
      <Button variant="ghost" size="icon" :aria-label="t('bulk.clear')" @click="selection.clear()">
        <X class="size-4" />
      </Button>
      <b class="min-w-0 flex-1 truncate">{{ t('bulk.picked', { count: selection.count.value }) }}</b>
      <Button variant="ghost" size="icon" :aria-label="t('bulk.change')" @click="sheetOpen = true">
        <Pencil class="size-4" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        :aria-label="t('bulk.delete')"
        :disabled="remove.isPending.value"
        @click="remove.mutate([...selection.ids.value])"
      >
        <Trash2 class="size-4" />
      </Button>
    </div>
    <span v-if="totals" class="text-sm text-muted-foreground">{{ totals }}</span>
    <ActionSheet
      v-model:open="sheetOpen"
      :actions="sheetActions"
      :title="t('bulk.picked', { count: selection.count.value })"
      :description="totals"
    />
  </div>
  <div v-else class="flex flex-wrap items-center gap-x-3 gap-y-2 rounded-xl bg-primary/12 px-3 py-2" data-tx-bulk>
    <Checkbox
      :model-value="true"
      :aria-label="t('bulk.clear')"
      class="border-primary"
      @click.stop="selection.clear()"
    />
    <b>{{ t('bulk.picked', { count: selection.count.value }) }}</b>
    <span v-if="totals" class="text-sm text-muted-foreground">{{ totals }}</span>
    <span class="flex-1" />
    <Button variant="outline" size="sm" @click="pick('category')">{{ t('bulk.category') }}</Button>
    <Button variant="outline" size="sm" :disabled="cantMove" :title="cantMove || undefined" @click="pick('store')">
      {{ t('bulk.wallet') }}
    </Button>
    <Button variant="outline" size="sm" :disabled="remove.isPending.value" @click="remove.mutate([...selection.ids.value])">
      <Trash2 class="size-4" />
      {{ t('bulk.delete') }}
    </Button>
    <span class="text-sm text-muted-foreground">{{ t('bulk.escHint') }}</span>
  </div>
  <BulkPickDialog
    v-model:open="pickOpen"
    :kind="pickKind"
    :count="selection.count.value"
    :pending="categorize.isPending.value || move.isPending.value"
    @pick="apply"
  />
</template>
