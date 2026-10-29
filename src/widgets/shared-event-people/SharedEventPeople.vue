<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { TriangleAlert } from 'lucide-vue-next'

import { ContactPicker } from '@/entities/contact'
import { ContactAccountForm } from '@/features/contact-edit'
import { GuestDetailsDialog } from '@/features/shared-event-edit'
import { sumDecimals, useSharedEvent } from '@/entities/shared-event'
import { useContacts } from '@/shared/api/queries'
import { formatDateRange } from '@/shared/lib/date-range'
import { Alert, AlertDescription } from '@/shared/ui/alert'
import { ConfirmDialog } from '@/shared/ui/confirm-dialog'
import { SectionState } from '@/shared/ui/section-state'

import { usePeopleActions } from './model/use-people-actions'
import type { SharedEventPeopleProps } from './shared-event-people.types'
import { PeopleTable } from './ui/people-table'
import { ShareOverrideDialog } from './ui/share-override'

/**
 * The `People` section: who takes part with their share, what they bought and the saldo. Add a
 * contact or a guest, open a guest's contacts, fix a total by hand, make a guest a contact, remove.
 */
const props = defineProps<SharedEventPeopleProps>()
const { t, locale } = useI18n()

const event = useSharedEvent(() => props.eventId)
const view = computed(() => event.data.value)
const actions = usePeopleActions(view)
const decimals = computed(() => (view.value ? sumDecimals(view.value) : 2))

const contacts = useContacts()
const picked = ref('')
watch(picked, (id) => {
  if (!id) return
  const contact = (contacts.data.value ?? []).find((entry) => entry.id === id)
  if (contact) actions.addContact(contact)
  picked.value = ''
})

const eventLabel = computed(() => {
  const current = view.value?.event
  if (!current) return ''
  const dates = formatDateRange({ start: current.starts_on, end: current.ends_on }, locale.value)
  return t('sharedEvents.guestForm.ofEvent', { title: dates ? `${current.title} · ${dates}` : current.title })
})
</script>

<template>
  <div class="min-w-0">
    <SectionState :loading="event.isPending.value" :error="event.error.value" skeleton="table" @retry="event.refetch()">
      <div v-if="view" class="grid min-w-0 gap-3">
        <Alert v-if="view.overrides_invalid" class="border-warning" data-tour="shared-overrides-invalid">
          <TriangleAlert class="text-warning" />
          <AlertDescription>{{ t('sharedEvents.people.overridesInvalid') }}</AlertDescription>
        </Alert>
        <div class="grid gap-1.5" data-tour="shared-add-person">
          <ContactPicker
            v-model="picked"
            :placeholder="t('sharedEvents.edit.addPerson')"
            :aria-label="t('sharedEvents.edit.addPerson')"
            :guest-label="t('sharedEvents.edit.guest')"
            @guest="actions.addGuest"
          />
        </div>
        <PeopleTable
          :people="view.people"
          :adjustments="view.event.adjustments"
          :currency="view.event.currency"
          :decimals="decimals"
          :actions-of="actions.actionsOf"
        />
        <p class="text-xs text-muted-foreground">{{ t('sharedEvents.people.hint') }}</p>
      </div>
    </SectionState>

    <template v-if="view">
      <GuestDetailsDialog
        v-model:open="actions.details.open"
        :guest="actions.detailsGuest.value"
        :event-label="eventLabel"
        can-make-contact
        :pending="actions.detailsPending.value"
        @save="actions.saveDetails"
        @make-contact="actions.makeContact"
      >
        <template #requisite-form="{ submit, cancel }">
          <ContactAccountForm :groups="[]" @submit="submit" @cancel="cancel" />
        </template>
      </GuestDetailsDialog>
      <ShareOverrideDialog
        v-model:open="actions.override.open"
        :person="actions.overridePerson.value"
        :current="actions.overrideTotal.value"
        :currency="view.event.currency"
        :spent="view.totals.spent"
        :pending="actions.overridePending.value"
        @save="actions.saveOverride"
      />
      <ConfirmDialog
        v-model:open="actions.drop.open"
        overlay="shared-drop-person"
        :title="t('sharedEvents.people.dropTitle', { name: actions.dropPerson.value?.name ?? '' })"
        :description="t('sharedEvents.people.dropText')"
        :pending="actions.dropPending.value"
        @confirm="actions.confirmDrop()"
      />
    </template>
  </div>
</template>
