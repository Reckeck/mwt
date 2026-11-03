<script setup lang="ts">
import { ref } from 'vue'

import { ContactEditDialog } from '@/features/contact-edit'
import { ContactSheet } from '@/widgets/contact-sheet'
import { ContactTable } from '@/widgets/contact-table'
import { useContactSelection } from '@/shared/lib/contact-selection'

import { ContactAddButton } from './ui/contact-add-button'

/**
 * Contacts and debts: who owes whom, the table, and the picked contact's card at the right (`?contact=`).
 * Only composition: the table and the card are widgets, a new contact is the shared create dialog.
 */
const { select } = useContactSelection()
const createOpen = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="min-w-0" data-tour="section-content"><ContactTable @create="createOpen = true">
        <template #actions><ContactAddButton @create="createOpen = true" /></template>
      </ContactTable></div>
    <ContactSheet />
    <ContactEditDialog v-model:open="createOpen" @saved="(contact) => select(contact.id)" />
  </div>
</template>
