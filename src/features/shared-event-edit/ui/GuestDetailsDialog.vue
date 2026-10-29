<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Trash2 } from 'lucide-vue-next'

import { required, useFormValidation } from '@/shared/lib/form'
import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Textarea } from '@/shared/ui/textarea'

import { EMPTY_DETAILS } from '../shared-event-edit.constants'
import type { GuestDetailsDialogProps, GuestDetailsResult } from '../shared-event-edit.types'

/**
 * «Контакты гостя»: a guest has a name, a phone, a messenger, requisites and a note, kept in the
 * event only. The same fields as a contact; «Сделать контактом» moves them to the address book.
 * The caller saves (a draft in the dialog, or the event itself). The requisite form belongs to
 * another feature (contacts), so the layer above hands it in through `#requisite-form`.
 */
const props = defineProps<GuestDetailsDialogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ save: [result: GuestDetailsResult]; makeContact: [] }>()

const { t } = useI18n()
const name = ref('')
const phone = ref('')
const messenger = ref('')
const note = ref('')
const accounts = ref<ContactAccount[]>([])
const adding = ref(false)

watch(open, (now) => {
  if (!now || !props.guest) return
  const details = props.guest.details ?? EMPTY_DETAILS
  name.value = props.guest.name
  phone.value = details.phone
  messenger.value = details.messenger
  note.value = details.note
  accounts.value = [...details.requisites]
  adding.value = false
})

const form = useFormValidation(() => ({ name: required(name.value) }), { resetOn: open })

function addAccount(account: ContactAccount) {
  accounts.value = [...accounts.value, { ...account, id: account.id || crypto.randomUUID() }]
  adding.value = false
}

function save() {
  emit('save', {
    name: name.value.trim(),
    details: { phone: phone.value, messenger: messenger.value, note: note.value, requisites: accounts.value },
  })
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-guest-details"
    form-id="shared-guest-details-form"
    size="sm"
    :title="t('sharedEvents.guestForm.title', { name: props.guest?.name ?? '' })"
    :submit-label="t('actions.save')"
    :pending="props.pending"
    @submit="form.submit(save, $event)"
  >
    <FormField for="guest-name" :label="t('sharedEvents.guestForm.name')" required :error="form.error('name')" :hint="props.eventLabel">
      <Input id="guest-name" v-model="name" autocomplete="off" />
    </FormField>
    <FormField for="guest-phone" :label="t('sharedEvents.guestForm.phone')">
      <Input id="guest-phone" v-model="phone" type="tel" autocomplete="off" />
    </FormField>
    <FormField for="guest-messenger" :label="t('sharedEvents.guestForm.messenger')">
      <Input id="guest-messenger" v-model="messenger" autocomplete="off" spellcheck="false" />
    </FormField>

    <div class="grid gap-2">
      <div class="flex items-baseline gap-2">
        <h3 class="text-sm font-semibold">{{ t('sharedEvents.guestForm.requisites') }}</h3>
        <span v-if="accounts.length" class="text-xs text-muted-foreground">{{ t('sharedEvents.guestForm.requisitesCount', accounts.length) }}</span>
      </div>
      <ul v-if="accounts.length" class="grid gap-2">
        <li v-for="account in accounts" :key="account.id" class="flex items-center gap-3 rounded-lg border border-border p-2 pl-3">
          <div class="grid min-w-0 flex-1">
            <span class="truncate text-sm font-medium">{{ account.label || t(`detailKind.${account.kind}`) }}</span>
            <span class="truncate text-xs text-muted-foreground tabular-nums">{{ account.value }}</span>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            :aria-label="t('actions.delete')"
            @click="accounts = accounts.filter((entry) => entry.id !== account.id)"
          >
            <Trash2 class="size-4" />
          </Button>
        </li>
      </ul>
      <!-- A form inside this dialog's form: its submit must not reach the outer one. -->
      <div v-if="adding" @submit.stop>
        <slot name="requisite-form" :submit="addAccount" :cancel="() => (adding = false)" />
      </div>
      <Button v-else-if="$slots['requisite-form']" type="button" variant="outline" class="w-fit" @click="adding = true">
        <Plus class="size-4" />{{ t('sharedEvents.guestForm.addRequisite') }}
      </Button>
      <p class="text-xs text-muted-foreground">{{ t('sharedEvents.guestForm.notInBook') }}</p>
    </div>

    <FormField for="guest-note" :label="t('stores.note')">
      <Textarea id="guest-note" v-model="note" rows="2" :placeholder="t('contacts.notePlaceholder')" />
    </FormField>

    <template v-if="props.canMakeContact" #footer-start>
      <Button type="button" variant="outline" :disabled="props.pending" @click="emit('makeContact')">
        {{ t('sharedEvents.people.makeContact') }}
      </Button>
    </template>
  </FormDialog>
</template>
