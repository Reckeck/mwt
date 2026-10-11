<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ClipboardPaste, Copy, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Select } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'
import { CHAIN_LABELS, CHAINS } from '@/shared/lib/chains'
import { detectDetail } from '@/shared/lib/detect-detail'

import { CONTACT_KINDS, DETAIL_KINDS } from '../contact-edit.constants'
import type { ContactEditorProps } from '../contact-edit.types'
import { draftOf, useContactSave } from '../model/use-contact-save'

const props = defineProps<ContactEditorProps>()
const { t } = useI18n()

const save = useContactSave(() => props.contact.id)

/* --- fields edited in place; each saves when it loses focus --- */
const name = ref('')
const note = ref('')
watch(
  () => props.contact,
  (contact) => {
    name.value = contact.name
    note.value = contact.note
  },
  { immediate: true },
)

function commitName() {
  const next = name.value.trim()
  if (!next) {
    name.value = props.contact.name
    return
  }
  if (next !== props.contact.name) save.mutate(draftOf(props.contact, { name: next }))
}

function commitNote() {
  if (note.value.trim() !== props.contact.note) save.mutate(draftOf(props.contact, { note: note.value.trim() }))
}

function setKind(kind: string) {
  save.mutate(draftOf(props.contact, { kind: kind as ContactKind }))
}

function patchAccount(index: number, patch: Partial<ContactAccount>) {
  const accounts = props.contact.accounts.map((account, at) => (at === index ? { ...account, ...patch } : account))
  save.mutate(draftOf(props.contact, { accounts }))
}

function removeAccount(index: number) {
  save.mutate(draftOf(props.contact, { accounts: props.contact.accounts.filter((_, at) => at !== index) }))
}

/* --- one paste box: the kind and the network are guessed --- */
const pasted = ref('')
const guess = computed(() => (pasted.value.trim() ? detectDetail(pasted.value) : null))

function addPasted() {
  const detected = guess.value
  if (!detected) return
  const account: ContactAccount = {
    id: '',
    group: null,
    label: '',
    kind: detected.kind,
    value: detected.value,
    currency: null,
    network: detected.network,
    active: true,
  }
  save.mutate(draftOf(props.contact, { accounts: [...props.contact.accounts, account] }), {
    onSuccess: () => {
      pasted.value = ''
    },
  })
}

async function copy(value: string) {
  try {
    await navigator.clipboard.writeText(value)
    toast.success(t('contacts.copied'))
  } catch {
    toast.error(t('contacts.copyFailed'))
  }
}

const inlineInput =
  'h-8 border-transparent bg-transparent px-1.5 shadow-none hover:border-input focus-visible:border-input'
</script>

<template>
  <div class="grid gap-4 @container" data-tour="contact-editor">
    <div class="flex flex-wrap items-center gap-2">
      <Input
        v-model="name"
        :class="[inlineInput, 'h-9 min-w-0 flex-1 text-lg font-semibold']"
        :aria-label="t('contacts.name')"
        @blur="commitName"
        @keydown.enter="($event.target as HTMLInputElement).blur()"
      />
      <Select
        :model-value="props.contact.kind"
        class="h-8 w-40"
        :aria-label="t('contacts.kind')"
        @update:model-value="setKind"
      >
        <option v-for="entry in CONTACT_KINDS" :key="entry" :value="entry">{{ t(`contactKind.${entry}`) }}</option>
      </Select>
    </div>

    <Textarea
      v-model="note"
      rows="1"
      :class="[inlineInput, 'min-h-8 resize-none text-sm text-muted-foreground']"
      :placeholder="t('contacts.notePlaceholder')"
      :aria-label="t('stores.note')"
      @blur="commitNote"
    />

    <section class="grid gap-2">
      <p class="text-xs font-medium text-muted-foreground">{{ t('contacts.accounts') }}</p>
      <ul v-if="props.contact.accounts.length" class="grid gap-1.5">
        <li
          v-for="(account, index) in props.contact.accounts"
          :key="account.id || index"
          class="group grid items-center gap-2 rounded-md border border-border p-2 @lg:grid-cols-[8.5rem_1fr_10rem_auto]"
          :class="!account.active && 'opacity-60'"
        >
          <Select
            :model-value="account.kind"
            class="h-8"
            :aria-label="t('contacts.accountKind')"
            @update:model-value="(kind) => patchAccount(index, { kind: kind as DetailKind })"
          >
            <option v-for="entry in DETAIL_KINDS" :key="entry" :value="entry">{{ t(`detailKind.${entry}`) }}</option>
          </Select>
          <div class="flex min-w-0 items-center gap-1">
            <span class="min-w-0 flex-1 truncate font-mono text-xs" :title="account.value">{{ account.value }}</span>
            <Button
              variant="ghost"
              size="icon"
              class="size-7 shrink-0 text-muted-foreground"
              :title="t('contacts.copy')"
              :aria-label="t('contacts.copy')"
              @click="copy(account.value)"
            >
              <Copy class="size-3.5" />
            </Button>
          </div>
          <Input
            :model-value="account.label"
            :class="inlineInput"
            :placeholder="t('contacts.accountLabel')"
            @change="(event: Event) => patchAccount(index, { label: (event.target as HTMLInputElement).value.trim() })"
          />
          <div class="flex items-center gap-1">
            <Select
              v-if="account.kind === 'crypto_address'"
              :model-value="account.network ?? ''"
              class="h-8 w-32"
              :aria-label="t('form.network')"
              @update:model-value="(network) => patchAccount(index, { network: (network as Chain) || null })"
            >
              <option value="">{{ t('form.noNetwork') }}</option>
              <option v-for="chain in CHAINS" :key="chain" :value="chain">{{ CHAIN_LABELS[chain] }}</option>
            </Select>
            <Badge v-if="!account.active" variant="outline">{{ t('contacts.inactive') }}</Badge>
            <Button
              variant="ghost"
              size="icon"
              class="size-7 text-muted-foreground hover:text-destructive"
              :title="t('contacts.removeAccount')"
              :aria-label="t('contacts.removeAccount')"
              @click="removeAccount(index)"
            >
              <Trash2 class="size-3.5" />
            </Button>
          </div>
        </li>
      </ul>

      <form class="grid gap-1" @submit.prevent="addPasted">
        <div class="flex items-center gap-2">
          <Input
            v-model="pasted"
            class="h-9 flex-1 font-mono text-sm"
            spellcheck="false"
            autocomplete="off"
            :placeholder="t('contacts.pastePlaceholder')"
            :aria-label="t('contacts.pastePlaceholder')"
          />
          <Button type="submit" variant="outline" :disabled="!guess || save.isPending.value">
            <ClipboardPaste class="size-4" />
            {{ t('contacts.addAccount') }}
          </Button>
        </div>
        <p v-if="guess" class="text-xs text-muted-foreground">
          {{ t('contacts.detected', { kind: t(`detailKind.${guess.kind}`) }) }}
          <template v-if="guess.network"> · {{ CHAIN_LABELS[guess.network] }}</template>
        </p>
      </form>
    </section>
  </div>
</template>
