<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { txId, txKind, txMeta, txPrincipalLegs, txSearchText } from '@/shared/lib/transactions'
import { useFormValidation } from '@/shared/lib/form'
import { Checkbox } from '@/shared/ui/checkbox'

import type { TransactionLinkDialogProps } from './transaction-link.types'

const props = defineProps<TransactionLinkDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const txs = useQuery({
  queryKey: ['transactions', ''],
  queryFn: () => call<Transaction[]>('list_transactions', { filter: null }),
})

const search = ref('')
const picked = ref<string[]>([])
watch(open, (isOpen) => {
  if (isOpen) {
    search.value = ''
    picked.value = []
  }
})

/** Nearby operations first: a chain is usually a few hours to days long. */
const candidates = computed(() => {
  const own = txMeta(props.tx)
  const at = new Date(own.timestamp).getTime()
  const needle = search.value.trim().toLowerCase()
  return (txs.data.value ?? [])
    .filter((tx) => txId(tx) !== own.id && (!own.flow_id || txMeta(tx).flow_id !== own.flow_id))
    .filter((tx) => !needle || txSearchText(tx).toLowerCase().includes(needle))
    .sort(
      (left, right) =>
        Math.abs(new Date(txMeta(left).timestamp).getTime() - at) -
        Math.abs(new Date(txMeta(right).timestamp).getTime() - at),
    )
    .slice(0, 30)
})

function toggle(id: string) {
  picked.value = picked.value.includes(id) ? picked.value.filter((entry) => entry !== id) : [...picked.value, id]
}

const form = useFormValidation(() => ({ picked: picked.value.length ? null : t('validation.pickOperations') }), {
  resetOn: open,
})

const save = useMutation({
  mutationFn: () =>
    call<string>('link_transactions', {
      input: { ids: [txId(props.tx), ...picked.value], flow_id: txMeta(props.tx).flow_id },
    }),
  onSuccess: () => {
    for (const queryKey of [['transactions'], ['flows'], ['report']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'flows.linkFailed', success: 'flows.linked' },
})
</script>

<template>
  <Dialog overlay="tx-link" v-model:open="open" :title="t('flows.linkTitle')" size="lg">
    <div class="grid gap-3">
      <p class="text-sm text-muted-foreground">{{ t('flows.linkHint') }}</p>
      <Input v-model="search" :placeholder="t('flows.search')" />
      <p v-if="form.firstMessage()" class="text-xs text-destructive" role="alert">{{ form.firstMessage() }}</p>
      <ul class="grid max-h-80 gap-1 overflow-y-auto" :data-invalid="form.invalidAttr('picked')">
        <li v-for="tx in candidates" :key="txId(tx)">
          <label
            :for="`tx-link-${txId(tx)}`"
            class="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 text-sm hover:bg-accent"
          >
            <Checkbox :id="`tx-link-${txId(tx)}`" :model-value="picked.includes(txId(tx))" @update:model-value="toggle(txId(tx))" />
            <span class="w-32 shrink-0 text-xs tabular-nums text-muted-foreground">{{ formatDateTime(txMeta(tx).timestamp) }}</span>
            <span class="shrink-0">{{ t(`tx.kind.${txKind(tx)}`) }}</span>
            <span class="min-w-0 truncate tabular-nums">
              {{ txPrincipalLegs(tx).map((leg) => `${leg.amount} ${leg.asset.symbol}`).join(' → ') }}
            </span>
          </label>
        </li>
      </ul>
    </div>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button :disabled="save.isPending.value" @click="form.submit(save.mutate, $event)">{{ t('flows.link') }}</Button>
    </template>
  </Dialog>
</template>
