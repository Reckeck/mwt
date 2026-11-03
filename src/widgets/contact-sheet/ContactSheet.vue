<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { ContactEditDialog } from '@/features/contact-edit'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { useContacts } from '@/shared/api/queries'
import { useContactSelection } from '@/shared/lib/contact-selection'
import { useIsPhone } from '@/shared/lib/phone'
import { ConfirmDialog } from '@/shared/ui/confirm-dialog'
import { Drawer, DrawerContent, DrawerDescription, DrawerTitle } from '@/shared/ui/drawer'
import { Sheet, SheetContent, SheetDescription, SheetTitle } from '@/shared/ui/sheet'

import { useContactDelete } from './model/use-contact-delete'
import { useContactMenu } from './model/use-contact-menu'
import { useContactTransfer } from './model/use-contact-transfer'
import ContactSheetBody from './ui/ContactSheetBody.vue'

/**
 * The picked contact's card (`?contact=`). A desktop panel at the right, over the table without a dim layer,
 * so another row can be clicked to switch; a phone gets a bottom drawer at full height. Esc, «×» and the
 * system «Back» take the pick off the address.
 */
const { t } = useI18n()
const phone = useIsPhone()
const { selectedId, clear } = useContactSelection()
const contacts = useContacts()

const found = computed(() => (contacts.data.value ?? []).find((contact) => contact.id === selectedId.value) ?? null)
/** The last card shown stays in place while the panel slides out, so it does not empty first. */
const shown = ref<Contact | null>(null)
watch(
  found,
  (next) => {
    if (next) shown.value = next
  },
  { immediate: true },
)
const open = computed(() => Boolean(found.value))

const transfer = useContactTransfer(() => shown.value)
const editOpen = ref(false)
const deleteOpen = ref(false)
const remove = useContactDelete(() => {
  deleteOpen.value = false
  clear()
})
const menu = useContactMenu({
  contact: () => shown.value as Contact,
  blocked: transfer.blocked,
  transfer: transfer.start,
  edit: () => (editOpen.value = true),
  askDelete: () => (deleteOpen.value = true),
})

function onOpenChange(next: boolean) {
  if (!next) clear()
}
/** The panel is not modal: a click on the table beside it is a pick of another contact, not a dismissal. */
function keepOpen(event: Event) {
  event.preventDefault()
}
</script>

<template>
  <template v-if="shown">
    <Drawer v-if="phone" :open="open" @update:open="onOpenChange">
      <DrawerContent layer="contact-sheet" class="h-[calc(100dvh-var(--safe-top)-1.5rem)]" data-tour="contact-sheet">
        <div class="min-h-0 flex-1 overflow-y-auto px-4 pb-[calc(1.5rem+var(--safe-bottom))] pt-2 [scrollbar-gutter:stable]">
          <ContactSheetBody
            :key="shown.id"
            :contact="shown"
            :menu="menu"
            :blocked="transfer.blocked.value"
            @close="clear"
            @transfer="(account) => transfer.start({ effect: null, accountId: account?.id ?? null })"
          >
            <template #title><DrawerTitle class="truncate text-lg font-semibold">{{ shown.name }}</DrawerTitle></template>
            <template #description="{ text }"><DrawerDescription class="truncate text-sm text-muted-foreground">{{ text }}</DrawerDescription></template>
          </ContactSheetBody>
        </div>
      </DrawerContent>
    </Drawer>

    <Sheet v-else :open="open" :modal="false" @update:open="onOpenChange">
      <SheetContent
        side="right"
        overlay="contact-sheet"
        no-overlay
        hide-close
        class="gap-0 overflow-y-auto [scrollbar-gutter:stable] sm:max-w-[35rem]"
        data-tour="contact-sheet"
        @interact-outside="keepOpen"
      >
        <div class="p-5">
          <ContactSheetBody
            :key="shown.id"
            :contact="shown"
            :menu="menu"
            :blocked="transfer.blocked.value"
            @close="clear"
            @transfer="(account) => transfer.start({ effect: null, accountId: account?.id ?? null })"
          >
            <template #title><SheetTitle class="truncate text-xl">{{ shown.name }}</SheetTitle></template>
            <template #description="{ text }"><SheetDescription class="truncate text-sm">{{ text }}</SheetDescription></template>
          </ContactSheetBody>
        </div>
      </SheetContent>
    </Sheet>

    <ContactEditDialog v-model:open="editOpen" :contact="shown" @deleted="clear" />
    <ConfirmDialog
      v-model:open="deleteOpen"
      :title="t('contacts.delete')"
      :description="t('contacts.sheet.deleteHint', { name: shown.name })"
      :pending="remove.isPending.value"
      @confirm="remove.mutate(shown)"
    />
    <TransactionCreateDialog
      v-if="transfer.storeId.value"
      v-model:open="transfer.open.value"
      :store-id="transfer.storeId.value"
      :counterpart="transfer.counterpart.value"
      :debt="transfer.transfer.value.effect"
    />
  </template>
</template>
