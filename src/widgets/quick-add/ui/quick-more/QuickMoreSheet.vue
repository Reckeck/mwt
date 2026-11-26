<script setup lang="ts">
import { computed, ref, useSlots } from 'vue'
import { useI18n } from 'vue-i18n'

import { PendingAttachments } from '@/features/attachment-edit'
import { DebtSwitch } from '@/features/debt-manage'
import type { QuickKind } from '@/features/transaction-create'
import { useStoreTree } from '@/shared/api/queries'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { formatDateRange } from '@/shared/lib/date-range'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { AdaptivePanel } from '@/shared/ui/adaptive-panel'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

import { type QuickMore, type QuickMoreField, filledMoreCount, moreFieldsOf } from '../../lib/quick-more'
import { useOpenEvents } from '../../model/use-open-events'

/**
 * «More»: the rare fields of the quick form in one panel (a bottom sheet on a phone, a popover by
 * the button on a desktop), so the form itself stays the amount, the wallet, the category and the
 * date. Which fields show depends on the kind; the counterparty comes in the `contact` slot. The
 * trigger is the last, dashed chip of the form's row, naming the first few fields (board 533527).
 */
defineSlots<{ contact?(): unknown }>()
const props = defineProps<{ kind: QuickKind; feeError?: string; contactSide?: boolean }>()
const more = defineModel<QuickMore>({ required: true })
const open = ref(false)
const { t, locale } = useI18n()
const tree = useStoreTree()

const fields = computed(() => moreFieldsOf(props.kind, props.contactSide))
const slots = useSlots()
const ORDER: Array<QuickMoreField | 'contact'> = ['fee', 'files', 'contact', 'payee', 'event', 'property', 'network', 'debt']
const summary = computed(() =>
  ORDER.filter((field) => (field === 'contact' ? Boolean(slots.contact) : fields.value.includes(field)))
    .slice(0, 3)
    .map((field) => t(`quickAdd.moreName.${field}`))
    .join(', '),
)
const filled = computed(() => filledMoreCount(more.value, props.kind, props.contactSide))
const properties = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => node.store.kind === 'property' && !node.store.archived))
const events = useOpenEvents()
const eventLabel = (row: SharedEventSummary) => {
  const dates = formatDateRange({ start: row.starts_on, end: row.ends_on }, locale.value)
  return dates ? `${row.title} · ${dates}` : row.title
}
const networks = CHAINS.filter((chain) => chain !== 'fiat')
</script>

<template>
  <AdaptivePanel v-model:open="open" :title="t('actions.more')" overlay="quick-more" show-title>
    <template #trigger>
      <Button type="button" variant="outline" class="h-9 gap-1.5 rounded-full border-dashed px-3 text-sm font-normal text-muted-foreground" data-quick-more>
        {{ t('quickAdd.moreOf', { list: summary }) }}
        <Badge v-if="filled" variant="secondary">{{ filled }}</Badge>
      </Button>
    </template>
      <div class="grid gap-4">
        <slot name="contact" />
        <FormField v-if="fields.includes('fee')" for="quick-fee" :label="t('tx.fee')" :error="props.feeError">
          <AmountInput id="quick-fee" v-model="more.fee" :aria-invalid="Boolean(props.feeError) || undefined" :calculator="false" />
        </FormField>
        <FormField v-if="fields.includes('payee')" for="quick-payee" :label="props.kind === 'topup' ? t('tx.more.fromWhom') : t('tx.more.toWhom')">
          <Input id="quick-payee" v-model="more.payee" maxlength="120" autocomplete="off" />
        </FormField>
        <FormField v-if="fields.includes('property') && properties.length" for="quick-property" :label="t('tx.more.propertyLabel')">
          <Select id="quick-property" v-model="more.propertyId" :placeholder="t('quickAdd.noProperty')">
            <SelectItem value="">{{ t('quickAdd.noProperty') }}</SelectItem>
            <SelectItem v-for="node in properties" :key="node.store.id" :value="node.store.id">{{ node.store.name }}</SelectItem>
          </Select>
        </FormField>
        <FormField v-if="fields.includes('event') && events.length" for="quick-event" :label="t('tx.field.event')" :hint="t('tx.field.eventHint')">
          <Select id="quick-event" v-model="more.eventId" :aria-label="t('tx.field.event')">
            <SelectItem value="">{{ t('tx.field.noEvent') }}</SelectItem>
            <SelectItem v-for="row in events" :key="row.id" :value="row.id">{{ eventLabel(row) }}</SelectItem>
          </Select>
        </FormField>
        <template v-if="fields.includes('network')">
          <FormField for="quick-network" :label="t('tx.network')">
            <Select id="quick-network" v-model="more.network" :placeholder="t('quickAdd.noNetwork')">
              <SelectItem value="">{{ t('quickAdd.noNetwork') }}</SelectItem>
              <SelectItem v-for="chain in networks" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</SelectItem>
            </Select>
          </FormField>
          <FormField for="quick-hash" :label="t('tx.txHash')">
            <Input id="quick-hash" v-model="more.txHash" maxlength="200" autocomplete="off" spellcheck="false" />
          </FormField>
        </template>
        <DebtSwitch v-if="fields.includes('debt')" v-model="more.asDebt" />
        <FormField v-if="fields.includes('files')" :label="t('attachments.title')">
          <PendingAttachments v-model="more.files" />
        </FormField>
      </div>
    <template #footer>
      <Button type="button" @click="open = false">{{ t('actions.close') }}</Button>
    </template>
  </AdaptivePanel>
</template>
