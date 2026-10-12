<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, ChevronDown, Star } from 'lucide-vue-next'
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxPortal,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
  ComboboxVirtualizer,
} from 'reka-ui'

import { Label } from '@/shared/ui/label'
import { useSettingsStore } from '@/shared/store/settings'

import { CURRENCY_ROW_HEIGHT } from './currency-picker.constants'
import type { CurrencyOption, CurrencyPickerProps } from './currency-picker.types'
import { useCurrencyOptions } from './useCurrencyOptions'

/**
 * The one currency control of the app: a select-looking trigger that opens a
 * searchable, virtualized list of the whole catalog (fiat + thousands of crypto
 * tickers). Only the visible rows exist in the DOM, so there is no cap on how
 * many matches can be scrolled through.
 */
const props = withDefaults(defineProps<CurrencyPickerProps>(), {
  label: '',
  placeholder: '',
  disabled: false,
  kinds: () => ['fiat', 'crypto'],
  allowEmpty: false,
  extra: () => [],
  compact: false,
  id: '',
})

const selected = defineModel<string>({ default: '' })

/**
 * The «none» row. Reka throws on an item whose value is `''`, which left the
 * list half-rendered and the dialog under it unresponsive; the empty model
 * value is mapped to this code instead.
 */
const NONE = '\u0000none'
const picked = computed({
  get: () => selected.value || (props.allowEmpty ? NONE : ''),
  set: (value: string) => {
    selected.value = value === NONE ? '' : value
  },
})

const { t } = useI18n()
const settingsStore = useSettingsStore()
const generatedId = useId()
const triggerId = computed(() => props.id || generatedId)

const open = ref(false)
const search = ref('')
watch(open, (isOpen) => {
  if (isOpen) search.value = ''
})

/** The stored value stays selectable even when the catalog is short or still loading. */
const extra = computed(() => [...props.extra, ...(selected.value ? [selected.value] : [])])
const { filter } = useCurrencyOptions(() => props.kinds, extra)

const EMPTY: CurrencyOption = { code: NONE, kind: 'fiat', name: '', favorite: false }
const options = computed(() => {
  const rows = filter(search.value)
  return props.allowEmpty && !search.value.trim() ? [EMPTY, ...rows] : rows
})

const selectedName = computed(() => options.value.find((row) => row.code === selected.value)?.name ?? '')

const catalogNotice = computed(() => {
  if (settingsStore.currenciesNotice === 'builtin') return t('settings.currencyListBuiltin')
  if (settingsStore.currenciesNotice === 'cached') return t('settings.currencyListCached')
  return null
})

function onOpen(isOpen: boolean) {
  if (isOpen && !settingsStore.currencies) void settingsStore.loadCurrencies().catch(() => {})
}
</script>

<template>
  <div class="grid gap-1.5">
    <Label v-if="props.label" :for="triggerId">{{ props.label }}</Label>

    <ComboboxRoot
      v-model="picked"
      v-model:open="open"
      :disabled="props.disabled"
      :ignore-filter="true"
      open-on-click
      @update:open="onOpen"
    >
      <!-- Reka hardcodes `tabindex="-1"` and an English `aria-label` on its trigger;
           both are overridden: this button is the only tab stop while closed. -->
      <ComboboxAnchor as-child>
        <ComboboxTrigger as-child :tabindex="0" :aria-label="props.label || t('settings.currencySelect')">
          <button
            :id="triggerId"
            type="button"
            :aria-expanded="open"
            class="flex h-9 w-full min-w-0 items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50 data-[state=open]:border-primary"
          >
            <span v-if="selected" class="flex min-w-0 items-baseline gap-1.5">
              <span class="font-medium uppercase">{{ selected }}</span>
              <span v-if="selectedName && !props.compact" class="truncate text-xs text-muted-foreground">{{ selectedName }}</span>
            </span>
            <span v-else class="truncate text-muted-foreground">
              {{ props.placeholder || t('settings.currencySearchPlaceholder') }}
            </span>
            <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
          </button>
        </ComboboxTrigger>
      </ComboboxAnchor>

      <ComboboxPortal>
        <ComboboxContent
          position="popper"
          class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-56 overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-lg"
          :side-offset="4"
        >
          <div class="border-b border-border p-1.5">
            <ComboboxInput
              v-model="search"
              auto-focus
              autocomplete="off"
              spellcheck="false"
              :display-value="() => search"
              :placeholder="t('settings.currencySearchPlaceholder')"
              class="h-8 w-full rounded border border-input bg-transparent px-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
            />
          </div>

          <ComboboxViewport class="max-h-72 overflow-y-auto p-1">
            <ComboboxVirtualizer
              v-slot="{ option }"
              :options="options"
              :estimate-size="CURRENCY_ROW_HEIGHT"
              :overscan="8"
              :text-content="(row: CurrencyOption) => row.code"
            >
              <ComboboxItem
                :value="option.code"
                class="flex h-8 w-full cursor-pointer select-none items-center gap-2 rounded px-2 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
              >
                <Check v-if="option.code === picked" class="size-3.5 shrink-0 text-primary" aria-hidden="true" />
                <Star v-else-if="option.favorite" class="size-3.5 shrink-0 text-amber-500" aria-hidden="true" />
                <span v-else class="size-3.5 shrink-0" aria-hidden="true" />
                <template v-if="option.code !== NONE">
                  <span class="w-14 shrink-0 font-medium uppercase">{{ option.code }}</span>
                  <span class="min-w-0 flex-1 truncate text-xs text-muted-foreground">{{ option.name }}</span>
                  <span class="shrink-0 text-[10px] uppercase text-muted-foreground">
                    {{ t(`currency.kind.${option.kind}`) }}
                  </span>
                </template>
                <span v-else class="text-muted-foreground">{{ t('currency.none') }}</span>
              </ComboboxItem>
            </ComboboxVirtualizer>

            <p v-if="options.length === 0" class="px-2 py-3 text-center text-xs text-muted-foreground">
              {{ settingsStore.currenciesLoading ? t('settings.currencyLoading') : t('settings.currencyNoMatches') }}
            </p>
          </ComboboxViewport>
        </ComboboxContent>
      </ComboboxPortal>
    </ComboboxRoot>

    <p v-if="catalogNotice" class="text-xs text-muted-foreground">{{ catalogNotice }}</p>
  </div>
</template>
