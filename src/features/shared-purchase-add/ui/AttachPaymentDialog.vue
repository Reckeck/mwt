<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { useAttachSharedPayment } from '@/entities/shared-event'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { required, useFormValidation } from '@/shared/lib/form'
import { txId, txKind, txMeta, txPrincipalLegs, txSearchText } from '@/shared/lib/transactions'
import { Alert } from '@/shared/ui/alert'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { Select, SelectItem } from '@/shared/ui/select'

import type { AttachPaymentDialogProps } from '../shared-purchase-add.types'

/**
 * `Link a payment`: a payment already in the journal (typed in, or imported) becomes a purchase of
 * this event. It stays one payment; the event only points at it. One payment belongs to one event.
 */
const props = defineProps<AttachPaymentDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const NEW_ITEM = 'new'

const payments = useQuery({
  queryKey: ['transactions', 'attach-to-event'],
  queryFn: () => call<Transaction[]>('list_transactions', { filter: null }),
  enabled: open,
})
const links = useQuery({
  queryKey: ['shared-events', 'tx-links'],
  queryFn: () => call<SharedTxLink[]>('list_shared_tx_links'),
  enabled: open,
})

const search = ref('')
const picked = ref('')
const itemChoice = ref(NEW_ITEM)
const title = ref('')

watch(open, (now) => {
  if (!now) return
  search.value = ''
  picked.value = ''
  itemChoice.value = NEW_ITEM
  title.value = ''
})

const candidates = computed(() => {
  const taken = new Set((links.data.value ?? []).map((link) => link.tx_id))
  const needle = search.value.trim().toLowerCase()
  return (payments.data.value ?? [])
    .filter((tx) => txKind(tx) === 'payment' && !taken.has(txId(tx)))
    .filter((tx) => txPrincipalLegs(tx).every((leg) => leg.asset.chain === 'fiat'))
    .filter((tx) => !needle || txSearchText(tx).toLowerCase().includes(needle))
    .sort((left, right) => txMeta(right).timestamp.localeCompare(txMeta(left).timestamp))
    .slice(0, 30)
})

/** What each row shows, worked out once. */
const rows = computed(() =>
  candidates.value.map((tx) => ({
    id: txId(tx),
    at: formatDateTime(txMeta(tx).timestamp),
    text: txMeta(tx).comment || t('tx.kind.payment'),
    amount: txPrincipalLegs(tx)
      .map((leg) => `${leg.amount} ${leg.asset.symbol}`)
      .join(' → '),
  })),
)

const form = useFormValidation(
  () => ({
    picked: required(picked.value, 'pick'),
    title: itemChoice.value === NEW_ITEM ? required(title.value) : null,
  }),
  { resetOn: open },
)

const attach = useAttachSharedPayment()
function save() {
  attach.mutate(
    {
      event_id: props.event.event.id,
      tx_id: picked.value,
      item_id: itemChoice.value === NEW_ITEM ? null : itemChoice.value,
      new_item_title: itemChoice.value === NEW_ITEM ? title.value.trim() : null,
      qty: null,
    },
    { onSuccess: () => (open.value = false) },
  )
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-attach"
    form-id="shared-attach-form"
    size="md"
    :title="t('sharedEvents.attach.title')"
    :description="t('sharedEvents.attach.description')"
    :submit-label="t('sharedEvents.attach.submit')"
    :pending="attach.isPending.value"
    @submit="form.submit(save, $event)"
  >
    <FormField :label="t('sharedEvents.attach.payment')" required :error="form.error('picked')">
      <div class="grid gap-2">
        <Input v-model="search" :placeholder="t('flows.search')" :aria-label="t('flows.search')" />
        <RadioGroup v-model="picked" class="grid max-h-64 gap-1 overflow-y-auto [scrollbar-gutter:stable]">
          <Label
            v-for="row in rows"
            :key="row.id"
            :for="`attach-${row.id}`"
            class="flex min-h-10 cursor-pointer items-center gap-x-3 rounded-md px-2 font-normal hover:bg-accent
              max-md:grid max-md:grid-cols-[auto_minmax(0,1fr)_auto] max-md:gap-y-0.5 max-md:py-2"
          >
            <RadioGroupItem :id="`attach-${row.id}`" :value="row.id" class="max-md:row-span-2" />
            <span class="w-36 shrink-0 text-xs tabular-nums text-muted-foreground max-md:col-start-2 max-md:w-auto">{{ row.at }}</span>
            <span class="min-w-0 flex-1 truncate max-md:col-span-2 max-md:col-start-2">{{ row.text }}</span>
            <span class="shrink-0 tabular-nums max-md:col-start-3 max-md:row-start-1">{{ row.amount }}</span>
          </Label>
        </RadioGroup>
        <p v-if="!rows.length && !payments.isPending.value" class="text-sm text-muted-foreground">{{ t('sharedEvents.attach.none') }}</p>
      </div>
    </FormField>
    <FormField :label="t('sharedEvents.attach.item')">
      <Select v-model="itemChoice" :aria-label="t('sharedEvents.attach.item')">
        <SelectItem :value="NEW_ITEM">{{ t('sharedEvents.attach.newItem') }}</SelectItem>
        <SelectItem v-for="item in props.event.event.items.filter((entry) => !entry.cancelled)" :key="item.id" :value="item.id">
          {{ item.title }}
        </SelectItem>
      </Select>
    </FormField>
    <FormField v-if="itemChoice === NEW_ITEM" for="attach-title" :label="t('sharedEvents.item.title')" required :error="form.error('title')">
      <Input id="attach-title" v-model="title" autocomplete="off" />
    </FormField>
    <Alert v-if="!props.event.event.i_participate">{{ t('sharedEvents.attach.notParticipating') }}</Alert>
  </FormDialog>
</template>
