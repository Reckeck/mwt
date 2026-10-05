<script setup lang="ts">
import { computed, ref, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, ChevronDown } from 'lucide-vue-next'
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
} from 'reka-ui'

import Label from '@/components/ui/label.vue'
import { useSettingsStore } from '@/stores/settings'

/**
 * Upper bound on rendered options.
 *
 * The catalog is the whole of CoinGecko `/coins/list` — thousands of tickers —
 * so an unfiltered list would build thousands of DOM nodes inside an already
 * open panel. Typing narrows the list long before this bites; when matches are
 * truncated, the panel says how many were hidden instead of silently lying.
 */
const MAX_OPTIONS = 300

const props = withDefaults(
  defineProps<{
    /** Label above the control; pass `''` to hide. */
    label?: string
    placeholder?: string
    disabled?: boolean
  }>(),
  { label: '', placeholder: '', disabled: false },
)

const selected = defineModel<string>({ default: '' })

const { t } = useI18n()
const settingsStore = useSettingsStore()
const triggerId = useId()

const open = ref(false)
const search = ref('')

/** Prefix matches first — `ru` should surface RUB before RU-something-else. */
const matches = computed(() => {
  const all = settingsStore.allCurrencies
  const needle = search.value.trim().toLowerCase()
  if (!needle) return all
  const byPrefix: string[] = []
  const bySubstring: string[] = []
  for (const code of all) {
    if (code.startsWith(needle)) byPrefix.push(code)
    else if (code.includes(needle)) bySubstring.push(code)
  }
  return [...byPrefix, ...bySubstring]
})

const options = computed(() => matches.value.slice(0, MAX_OPTIONS))
const hiddenCount = computed(() => matches.value.length - options.value.length)

/**
 * Search text drives the catalog fetch.
 *
 * One keystroke is enough to start it: the backend returns the whole catalog in
 * a single request (network → cache → built-in list), so waiting for a longer,
 * more specific query would gain nothing.
 */
function onSearch(value: string) {
  search.value = value
  if (!settingsStore.currencies) {
    void settingsStore.loadCurrencies().catch(() => {
      /* the notice below already explains a short list */
    })
  }
}

const catalogNotice = computed(() => {
  if (settingsStore.currenciesNotice === 'builtin') return t('settings.currencyListBuiltin')
  if (settingsStore.currenciesNotice === 'cached') return t('settings.currencyListCached')
  return null
})
</script>

<template>
  <div class="grid gap-1.5">
    <Label v-if="props.label" :for="triggerId">{{ props.label }}</Label>

    <ComboboxRoot
      v-model="selected"
      v-model:open="open"
      :disabled="props.disabled"
      :ignore-filter="true"
      open-on-click
    >
      <!-- `ComboboxAnchor` registers the popper reference, `ComboboxTrigger` toggles
         the panel. Both are `as-child`, so the DOM stays a single <button>.
         Reka hardcodes `tabindex="-1"` and an English `aria-label` on its
         trigger, so both are overridden here: this control is the only tab stop
         while the panel is closed, and the UI is bilingual. -->
      <ComboboxAnchor as-child>
        <ComboboxTrigger as-child :tabindex="0" :aria-label="props.label || t('settings.currencySelect')">
          <button
            :id="triggerId"
            type="button"
            :aria-expanded="open"
            class="flex h-9 w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50 data-[state=open]:border-primary"
          >
            <span class="truncate" :class="!selected && 'text-muted-foreground'">
              {{ selected ? selected.toUpperCase() : t('settings.currencySearchPlaceholder') }}
            </span>
            <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
          </button>
        </ComboboxTrigger>
      </ComboboxAnchor>

      <ComboboxPortal>
        <!-- `position` has no default in Reka, so without it the panel renders inline
             at the bottom of the page instead of under the trigger. The width
             var is `--reka-combobox-trigger-width` (no `content-` in it). -->
        <ComboboxContent
          position="popper"
          class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-44 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg"
          :side-offset="4"
        >
          <div class="border-b border-border p-1.5">
            <ComboboxInput
              auto-focus
              autocomplete="off"
              spellcheck="false"
              :display-value="() => ''"
              :placeholder="t('settings.currencySearchPlaceholder')"
              class="h-8 w-full rounded border border-input bg-transparent px-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              @update:model-value="onSearch"
            />
          </div>

          <ComboboxViewport class="max-h-64 overflow-y-auto p-1">
            <ComboboxItem
              v-for="code in options"
              :key="code"
              :value="code"
              class="flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm uppercase outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
            >
              <Check v-if="code === selected" class="size-3.5 shrink-0 text-primary" aria-hidden="true" />
              <span v-else class="size-3.5 shrink-0" aria-hidden="true" />
              <span class="truncate">{{ code }}</span>
            </ComboboxItem>

            <p v-if="options.length === 0" class="px-2 py-3 text-center text-xs text-muted-foreground">
              {{ t('settings.currencyNoMatches') }}
            </p>
          </ComboboxViewport>

          <p v-if="hiddenCount > 0" class="border-t border-border px-2 py-1.5 text-xs text-muted-foreground">
            {{ t('settings.currencyMatchLimit', { count: hiddenCount }) }}
          </p>
        </ComboboxContent>
      </ComboboxPortal>
    </ComboboxRoot>

    <p v-if="catalogNotice" class="text-xs text-muted-foreground">{{ catalogNotice }}</p>
  </div>
</template>