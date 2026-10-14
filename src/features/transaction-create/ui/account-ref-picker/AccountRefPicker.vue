<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, ChevronDown } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import {
  ComboboxAnchor,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
} from '@/shared/ui/combobox'
import { Input } from '@/shared/ui/input'

import type { AccountOption, AccountRefPickerProps } from './account-ref-picker.types'
import { NONE, RAW, encode, iconOf, useAccountOptions } from './use-account-options'

/**
 * Who the money moves with, picked like a currency: a searchable list where
 * each row says what it is on top and the details under it — a wallet's path
 * and balance, a contact's card or address. Every side appears exactly once.
 * Built from the shadcn-vue combobox parts.
 */
const props = withDefaults(defineProps<AccountRefPickerProps>(), {
  stores: true,
  contacts: true,
  raw: true,
  excludeStoreId: null,
  emptyLabel: '',
  id: '',
})

const side = defineModel<AccountRef | null>({ default: null })

const { t } = useI18n()
const generatedId = useId()
const triggerId = computed(() => props.id || generatedId)

const rawText = ref(side.value && 'raw' in side.value ? side.value.raw : '')
const choice = ref(encode(side.value))

// Follow outside changes (a swap of the two ends, an edit loaded later).
watch(side, (value) => {
  // Free text not typed yet is still a free-text choice, not «not set».
  const keepRaw = value === null && choice.value === RAW
  if (!keepRaw && encode(value) !== choice.value) choice.value = encode(value)
  if (value && 'raw' in value && value.raw !== rawText.value) rawText.value = value.raw
})

function commit() {
  const [kind, id, account] = choice.value.split(':')
  if (kind === 'store' && id) side.value = { store: id }
  else if (kind === 'contact' && id) side.value = { contact: { contact_id: id, account_id: account ?? null } }
  else if (choice.value === RAW) side.value = rawText.value.trim() ? { raw: rawText.value.trim() } : null
  else side.value = null
}
watch(choice, commit)
watch(rawText, () => {
  if (choice.value === RAW) commit()
})

const allOptions = useAccountOptions(props)

const open = ref(false)
const search = ref('')
watch(open, (isOpen) => {
  if (isOpen) search.value = ''
})
/** A matching account keeps its contact on top, so it is clear whose card it is. */
const shown = computed(() => {
  const needle = search.value.trim().toLowerCase()
  if (!needle) return allOptions.value
  const hit = (row: AccountOption) => row.value === RAW || row.haystack.includes(needle)
  const parents = new Set(allOptions.value.filter((row) => row.parent && hit(row)).map((row) => row.parent))
  return allOptions.value.filter((row) => hit(row) || parents.has(row.value))
})
/** Rows under their headings, in first-seen order. */
const groups = computed(() => {
  const out = new Map<string, AccountOption[]>()
  for (const row of shown.value) out.set(row.group, [...(out.get(row.group) ?? []), row])
  return [...out.entries()]
})

const current = computed(() => allOptions.value.find((row) => row.value === choice.value) ?? null)
</script>

<template>
  <div class="grid gap-1.5">
    <ComboboxRoot v-model="choice" v-model:open="open" :ignore-filter="true" open-on-click>
      <ComboboxAnchor as-child class="w-full">
        <ComboboxTrigger as-child :aria-label="t('tx.field.pickSide')">
          <Button
            :id="triggerId"
            variant="outline"
            :aria-expanded="open"
            class="h-auto min-h-9 w-full min-w-0 justify-between gap-2 px-3 py-1 text-left font-normal data-[state=open]:border-primary"
          >
            <span v-if="current" class="grid min-w-0 leading-tight">
              <span class="truncate font-medium">{{ current.title }}</span>
              <span class="truncate text-xs text-muted-foreground">{{ current.detail }}</span>
            </span>
            <span v-else class="truncate text-muted-foreground">
              {{ props.emptyLabel || t('tx.field.pickSide') }}
            </span>
            <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
          </Button>
        </ComboboxTrigger>
      </ComboboxAnchor>

      <!-- Above dialogs (z-50): the operation form lives in one. -->
      <ComboboxList class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-72" align="start">
        <ComboboxInput
          v-model="search"
          auto-focus
          autocomplete="off"
          spellcheck="false"
          :display-value="() => search"
          :placeholder="t('tx.side.search')"
        />
        <ComboboxViewport class="max-h-80 p-1">
          <ComboboxItem v-if="props.emptyLabel && !search.trim()" :value="NONE" class="text-muted-foreground">
            {{ props.emptyLabel }}
          </ComboboxItem>
          <ComboboxGroup v-for="[group, rows] in groups" :key="group" :heading="group || undefined" class="p-0">
            <!-- A contact's accounts hang under it, indented, instead of repeating its name. -->
            <ComboboxItem
              v-for="row in rows"
              :key="row.value"
              :value="row.value"
              class="w-full cursor-pointer"
              :class="row.parent && 'ml-4 w-[calc(100%-1rem)] rounded-l-none border-l border-border py-1'"
            >
              <Check v-if="row.value === choice" class="size-3.5 text-primary" aria-hidden="true" />
              <component
                :is="iconOf(row)"
                v-else
                class="size-3.5 text-muted-foreground"
                :style="row.color ? { color: row.color } : undefined"
                aria-hidden="true"
              />
              <span class="grid min-w-0 leading-tight">
                <span class="truncate" :class="row.parent ? 'text-[13px]' : 'font-medium'">{{ row.title }}</span>
                <span class="truncate text-xs text-muted-foreground">{{ row.detail }}</span>
              </span>
            </ComboboxItem>
          </ComboboxGroup>
          <p v-if="!shown.length" class="px-2 py-3 text-center text-xs text-muted-foreground">{{ t('tx.side.noMatches') }}</p>
        </ComboboxViewport>
      </ComboboxList>
    </ComboboxRoot>
    <Input v-if="choice === RAW" v-model="rawText" :placeholder="t('tx.side.otherHint')" />
  </div>
</template>
