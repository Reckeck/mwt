<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { SlidersHorizontal } from 'lucide-vue-next'

import { PendingAttachments } from '@/features/attachment-edit'
import { DebtSwitch } from '@/features/debt-manage'
import type { QuickKind } from '@/features/transaction-create'
import { useStoreTree } from '@/shared/api/queries'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { formatDateRange } from '@/shared/lib/date-range'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { AmountInput } from '@/shared/ui/amount-input'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { Sheet, SheetContent, SheetFooter, SheetHeader, SheetTitle, SheetTrigger } from '@/shared/ui/sheet'

import { type QuickMore, filledMoreCount, moreFieldsOf } from '../../lib/quick-more'
import { useOpenEvents } from '../../model/use-open-events'

/**
 * «More»: the rare fields of the quick form in one bottom sheet, so the form itself stays the
 * amount, the wallet, the category and the date. Which fields show depends on the kind.
 */
const props = defineProps<{ kind: QuickKind; feeError?: string; contactSide?: boolean }>()
const more = defineModel<QuickMore>({ required: true })
const open = ref(false)
const { t, locale } = useI18n()
const tree = useStoreTree()

const fields = computed(() => moreFieldsOf(props.kind, props.contactSide))
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
  <Sheet v-model:open="open">
    <SheetTrigger as-child>
      <Button type="button" size="sm" variant="outline" class="w-fit rounded-full" data-quick-more>
        <SlidersHorizontal class="size-4" />{{ t('actions.more') }}
        <Badge v-if="filled" variant="secondary">{{ filled }}</Badge>
      </Button>
    </SheetTrigger>
    <SheetContent side="bottom" overlay="quick-more" class="max-h-[85dvh] gap-3 px-4 pt-4" :aria-describedby="undefined">
      <SheetHeader class="p-0"><SheetTitle>{{ t('actions.more') }}</SheetTitle></SheetHeader>
      <div class="grid min-h-0 flex-1 gap-4 overflow-y-auto pb-2">
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
      <SheetFooter class="p-0 pb-2">
        <Button type="button" @click="open = false">{{ t('actions.close') }}</Button>
      </SheetFooter>
    </SheetContent>
  </Sheet>
</template>
