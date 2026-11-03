<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { contactNets } from '@/entities/contact'
import { ContactAccounts } from '@/features/contact-edit'
import { useContactBalances } from '@/shared/api/queries'
import { useContactSharedEvents } from '@/entities/shared-event'
import { useContactSummaries } from '@/shared/api/queries'
import type { UiAction } from '@/shared/ui/context-menu'
import { Tabs } from '@/shared/ui/tabs'

import ContactEventsTab from './ContactEventsTab.vue'
import ContactOperationsTab from './ContactOperationsTab.vue'
import ContactSheetHeader from './ContactSheetHeader.vue'
import ContactSummaryCards from './ContactSummaryCards.vue'

/** What the card holds, the same on a desktop panel and a phone drawer: header, three figures, three tabs. */
const props = defineProps<{ contact: Contact; menu: UiAction[]; blocked: string | false }>()
const emit = defineEmits<{ close: []; transfer: [account: ContactAccount | null] }>()

const { t } = useI18n()
const balances = useContactBalances()
const summaries = useContactSummaries()
const events = useContactSharedEvents(() => props.contact.id)

const nets = computed(() => contactNets(balances.data.value ?? [], props.contact.id))
const tab = ref('operations')
const txCount = computed(() => summaries.data.value?.find((row) => row.contact_id === props.contact.id)?.tx_count ?? 0)
const tabs = computed(() => [
  { value: 'operations', label: `${t('contacts.operations')} ${txCount.value}` },
  { value: 'requisites', label: `${t('contacts.accounts')} ${props.contact.accounts.length}` },
  { value: 'events', label: `${t('contacts.sharedEvents.title')} ${events.data.value?.length ?? 0}` },
])
</script>

<template>
  <div class="grid min-w-0 grid-cols-1 content-start gap-4" data-tour="contact-editor">
    <ContactSheetHeader :contact="props.contact" :menu="props.menu" :blocked="props.blocked" @close="emit('close')" @transfer="emit('transfer', null)">
      <template #title><slot name="title" /></template>
      <template #description="{ text }"><slot name="description" :text="text" /></template>
    </ContactSheetHeader>
    <ContactSummaryCards :contact-id="props.contact.id" :nets="nets" />
    <Tabs v-model="tab" :tabs="tabs" fill data-tour="contact-sheet-tabs">
      <template #operations>
        <ContactOperationsTab :contact-id="props.contact.id" :transfer-blocked="props.blocked" @transfer="emit('transfer', null)" />
      </template>
      <template #requisites>
        <ContactAccounts :contact="props.contact" :transfer-disabled="props.blocked" @transfer="(account) => emit('transfer', account)" />
      </template>
      <template #events><ContactEventsTab :contact-id="props.contact.id" /></template>
    </Tabs>
  </div>
</template>
