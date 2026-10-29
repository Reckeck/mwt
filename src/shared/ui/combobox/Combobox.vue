<script setup lang="ts">
import { computed, ref } from 'vue'
import { Check, ChevronDown, Plus, UserRound, X } from 'lucide-vue-next'
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxGroup,
  ComboboxInput,
  ComboboxItem,
  ComboboxLabel,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
} from 'reka-ui'

import type { ComboboxOption, ComboboxProps } from './combobox.types'

const props = defineProps<ComboboxProps>()
const emit = defineEmits<{ create: [text: string]; guest: [text: string] }>()

/** A string in single mode, a list in `multiple` mode. */
const model = defineModel<string | string[]>({ required: true })

/** Value of the synthetic "create" item; never stored in the model. */
const CREATE_VALUE = '\u0000create'
const GUEST_VALUE = '\u0000guest'

const open = ref(false)
const search = ref('')

const picked = computed<string[]>(() => {
  if (Array.isArray(model.value)) return model.value
  return model.value ? [model.value] : []
})

const labelOf = (value: string) => props.options.find((option) => option.value === value)?.label ?? value

const needle = computed(() => search.value.trim().toLowerCase())

/** Prefix matches first, then substring matches. */
const matches = computed(() => {
  if (!needle.value) return props.options
  const byPrefix: ComboboxOption[] = []
  const bySubstring: ComboboxOption[] = []
  for (const option of props.options) {
    const label = option.label.toLowerCase()
    if (label.startsWith(needle.value)) byPrefix.push(option)
    else if (label.includes(needle.value)) bySubstring.push(option)
  }
  return [...byPrefix, ...bySubstring]
})

const groups = computed(() => {
  const out: { key: string; label?: string; items: ComboboxOption[] }[] = []
  for (const option of matches.value) {
    const key = option.group ?? ''
    let group = out.find((entry) => entry.key === key)
    if (!group) {
      group = { key, label: option.groupLabel, items: [] }
      out.push(group)
    }
    group.items.push(option)
  }
  return out
})

const noExactMatch = computed(
  () => needle.value.length > 0 && !props.options.some((option) => option.label.toLowerCase() === needle.value),
)
const canCreate = computed(() => Boolean(props.createLabel) && noExactMatch.value)
const canGuest = computed(() => Boolean(props.guestLabel) && noExactMatch.value)

/** Reka hands back whatever it holds; the create sentinel is filtered out. */
function onPick(value: unknown) {
  if (props.multiple) {
    const values = (Array.isArray(value) ? value : []).filter(
      (entry): entry is string => typeof entry === 'string' && entry !== CREATE_VALUE && entry !== GUEST_VALUE,
    )
    model.value = values
  } else if (typeof value === 'string' && value !== CREATE_VALUE && value !== GUEST_VALUE) {
    model.value = value
    open.value = false
  }
  search.value = ''
}

function create() {
  const text = search.value.trim()
  if (!text) return
  emit('create', text)
  search.value = ''
}

function guest() {
  const text = search.value.trim()
  if (!text) return
  emit('guest', text)
  search.value = ''
  open.value = false
}

function remove(value: string) {
  if (props.multiple) model.value = picked.value.filter((entry) => entry !== value)
  else model.value = ''
}

/** Enter with no matching option creates the typed value right away. */
function onEnter(event: KeyboardEvent) {
  if (!canCreate.value || matches.value.length > 0) return
  event.preventDefault()
  create()
}

function onBackspace() {
  if (props.multiple && !search.value && picked.value.length) remove(picked.value[picked.value.length - 1])
}
</script>

<template>
  <ComboboxRoot
    :model-value="props.multiple ? picked : (picked[0] ?? '')"
    v-model:open="open"
    :multiple="props.multiple"
    :disabled="props.disabled"
    :ignore-filter="true"
    :reset-search-term-on-blur="false"
    open-on-click
    @update:model-value="onPick"
  >
    <ComboboxAnchor
      class="flex min-h-9 w-full flex-wrap items-center gap-1 rounded-md border border-input bg-transparent px-2 py-1 text-sm shadow-sm transition-colors focus-within:ring-1 focus-within:ring-ring data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50"
    >
      <template v-if="props.multiple">
        <span
          v-for="value in picked"
          :key="value"
          class="inline-flex items-center gap-1 rounded-full bg-primary px-2 py-0.5 text-xs font-medium text-primary-foreground"
        >
          {{ labelOf(value) }}
          <button
            type="button"
            class="rounded-full opacity-70 hover:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            :aria-label="`× ${labelOf(value)}`"
            @click.stop="remove(value)"
          >
            <X class="size-3" />
          </button>
        </span>
      </template>

      <ComboboxInput
        :id="props.id"
        v-model="search"
        autocomplete="off"
        spellcheck="false"
        :aria-label="props.ariaLabel"
        :placeholder="picked.length && props.multiple ? '' : props.placeholder"
        :display-value="props.multiple ? undefined : () => (search ? search : picked[0] ? labelOf(picked[0]) : '')"
        class="h-7 min-w-24 flex-1 bg-transparent px-1 outline-none placeholder:text-muted-foreground"
        @keydown.backspace="onBackspace"
        @keydown.enter="onEnter"
      />

      <ComboboxTrigger class="ml-auto shrink-0 text-muted-foreground" :tabindex="-1">
        <ChevronDown class="size-4" />
      </ComboboxTrigger>
    </ComboboxAnchor>

    <ComboboxPortal>
      <ComboboxContent
        position="popper"
        class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-44 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg"
        :side-offset="4"
      >
        <ComboboxViewport class="max-h-64 overflow-y-auto p-1">
          <ComboboxGroup v-for="group in groups" :key="group.key">
            <ComboboxLabel v-if="group.label" class="px-2 pt-2 pb-1 text-xs text-muted-foreground">
              {{ group.label }}
            </ComboboxLabel>
            <ComboboxItem
              v-for="option in group.items"
              :key="option.value"
              :value="option.value"
              :text-value="option.label"
              class="flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
            >
              <Check v-if="picked.includes(option.value)" class="size-3.5 shrink-0 text-primary" aria-hidden="true" />
              <span v-else class="size-3.5 shrink-0" aria-hidden="true" />
              <span class="truncate">{{ option.label }}</span>
            </ComboboxItem>
          </ComboboxGroup>

          <ComboboxItem
            v-if="canCreate && props.createLabel"
            :value="CREATE_VALUE"
            :text-value="search"
            class="flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm text-primary outline-none data-[highlighted]:bg-accent"
            @select.prevent="create"
          >
            <Plus class="size-3.5 shrink-0" aria-hidden="true" />
            <span class="truncate">{{ props.createLabel(search.trim()) }}</span>
          </ComboboxItem>

          <ComboboxItem
            v-if="canGuest && props.guestLabel"
            :value="GUEST_VALUE"
            :text-value="search"
            class="flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm text-primary outline-none data-[highlighted]:bg-accent"
            @select.prevent="guest"
          >
            <UserRound class="size-3.5 shrink-0" aria-hidden="true" />
            <span class="truncate">{{ props.guestLabel(search.trim()) }}</span>
          </ComboboxItem>

          <p
            v-if="matches.length === 0 && !canCreate && !canGuest"
            class="px-2 py-3 text-center text-xs text-muted-foreground"
          >
            {{ props.emptyLabel }}
          </p>
        </ComboboxViewport>
      </ComboboxContent>
    </ComboboxPortal>
  </ComboboxRoot>
</template>
