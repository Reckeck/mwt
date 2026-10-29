<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { ContactPicker } from '@/entities/contact'
import { isQuickContact } from '@/entities/contact'
import { PersonName } from '@/entities/shared-event'
import { useContacts } from '@/shared/api/queries'
import type { UiAction } from '@/shared/ui/context-menu'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { DraftPerson } from '../shared-event-edit.types'
import { hasGuestDetails } from '../model/guest-details'

/**
 * «Участники · N»: the people typed in so far — me first when I take part — as a short table that
 * scrolls inside itself (header pinned, the scroll lane reserved). Add by name or contact; a name
 * nobody has can become a contact on the spot or stay a guest.
 */
const props = defineProps<{ people: DraftPerson[]; includeMe: boolean }>()
const emit = defineEmits<{
  addContact: [contact: Contact]
  addGuest: [name: string]
  remove: [id: string]
  details: [id: string]
}>()

const { t } = useI18n()
const contacts = useContacts()
const picked = ref('')
// A contact just made in the picker is not in the list until it is read again: wait for it, do not drop the pick.
watch([picked, () => contacts.data.value], ([id]) => {
  if (!id) return
  const contact = (contacts.data.value ?? []).find((entry) => entry.id === id)
  if (!contact) return
  emit('addContact', contact)
  picked.value = ''
})

const count = computed(() => props.people.length + (props.includeMe ? 1 : 0))
const quickIds = computed(
  () => new Set((contacts.data.value ?? []).filter((contact) => isQuickContact(contact)).map((contact) => contact.id)),
)

function actionsOf(person: DraftPerson): UiAction[] {
  return [
    ...(person.contactId
      ? []
      : [{ id: 'details', label: t('sharedEvents.people.details'), run: () => emit('details', person.id) }]),
    { id: 'remove', label: t('actions.delete'), destructive: true, run: () => emit('remove', person.id) },
  ]
}
</script>

<template>
  <div class="grid gap-2" data-tour="shared-edit-people">
    <h3 class="text-sm font-medium">{{ t('sharedEvents.edit.people', { count }) }}</h3>
    <ContactPicker
      v-model="picked"
      :placeholder="t('sharedEvents.edit.addPerson')"
      :aria-label="t('sharedEvents.edit.addPerson')"
      :guest-label="t('sharedEvents.edit.guest')"
      @guest="emit('addGuest', $event)"
    />
    <Table
      container-class="max-h-[19.5rem] rounded-lg border border-border [scrollbar-gutter:stable] max-md:max-h-[14.5rem]"
    >
      <TableHeader class="sticky top-0 z-10 bg-card shadow-[0_1px_0_var(--border)]">
        <TableRow>
          <TableHead class="text-xs text-muted-foreground">{{ t('sharedEvents.people.name') }}</TableHead>
          <TableHead class="w-12"><span class="sr-only">{{ t('actions.more') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="props.includeMe">
          <TableCell class="px-2 py-1.5"><PersonName name="" kind="me" /></TableCell>
          <TableCell class="w-12 px-2" />
        </TableRow>
        <TableRow v-for="person in props.people" :key="person.id">
          <TableCell class="px-2 py-1.5">
            <PersonName
              :name="person.name"
              :kind="person.contactId ? 'contact' : 'guest'"
              :has-details="hasGuestDetails(person.details)"
              :quick="person.contactId ? quickIds.has(person.contactId) : false"
            />
          </TableCell>
          <TableCell class="w-12 px-2">
            <ActionMenu :actions="actionsOf(person)" :label="t('actions.more')" :sheet="{ title: person.name }" />
          </TableCell>
        </TableRow>
        <TableRow v-if="!count">
          <TableCell colspan="2" class="px-3 py-4 text-center text-sm text-muted-foreground">
            {{ t('sharedEvents.edit.noPeople') }}
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
