<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Check, ChevronDown, Star } from 'lucide-vue-next'
import { ComboboxVirtualizer } from 'reka-ui'

import { Button } from '@/shared/ui/button'
import {
  ComboboxAnchor,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxRoot,
  ComboboxTrigger,
  ComboboxViewport,
} from '@/shared/ui/combobox'
import { Label } from '@/shared/ui/label'
import { useSettingsStore } from '@/shared/store/settings'
import { currencySymbol } from '@/shared/lib/format'

import { CURRENCY_ROW_HEIGHT } from './currency-picker.constants'
import type { CurrencyKind, CurrencyOption, CurrencyPickerProps } from './currency-picker.types'
import { useCurrencyOptions } from './useCurrencyOptions'

/**
 * The one currency control of the app: a select-looking trigger that opens a
 * searchable, virtualized list of the whole catalog (fiat + thousands of crypto
 * tickers). Built from the shadcn-vue combobox parts; only the list is Reka's
 * virtualizer (shadcn has none), so only the visible rows exist in the DOM.
 */
const props = withDefaults(defineProps<CurrencyPickerProps>(), {
  label: '',
  placeholder: '',
  disabled: false,
  kinds: () => ['fiat', 'crypto'],
  allowEmpty: false,
  extra: () => [],
  compact: false,
  chip: false,
  id: '',
  required: false,
  error: undefined,
})

const selected = defineModel<string>({ default: '' })

/**
 * The «none» row. Reka throws on an item whose value is `''`, which left the
 * list half-rendered and the dialog under it unresponsive; the empty model
 * value is mapped to this code instead.
 */
const NONE = '\u0000none'
/** Values of the group title rows; never picked, never a currency code. */
const GROUP_PREFIX = '\u0000group:'
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
const header = (group: 'favorites' | CurrencyKind): CurrencyOption => ({
  code: `${GROUP_PREFIX}${group}`,
  kind: 'fiat',
  name: t(`currency.group.${group}`),
  favorite: false,
  header: true,
})

/** With no search the list is grouped: favourites (the base first), then currencies, then crypto. */
const options = computed(() => {
  const rows = filter(search.value)
  if (search.value.trim()) return rows
  const favorites = rows.filter((row) => row.favorite)
  const groups: CurrencyOption[] = []
  if (favorites.length) groups.push(header('favorites'), ...favorites)
  for (const kind of ['fiat', 'crypto'] as const) {
    const rest = rows.filter((row) => !row.favorite && row.kind === kind)
    if (rest.length) groups.push(header(kind), ...rest)
  }
  return props.allowEmpty ? [EMPTY, ...groups] : groups
})

const selectedName = computed(() => options.value.find((row) => row.code === selected.value)?.name ?? '')

const catalogNotice = computed(() => {
  if (settingsStore.currenciesNotice === 'builtin') return t('settings.currencyListBuiltin')
  if (settingsStore.currenciesNotice === 'cached') return t('settings.currencyListCached')
  return null
})

/** A no-op once the full catalog is in; a degraded one is retried after a minute. */
function onOpen(isOpen: boolean) {
  if (isOpen) void settingsStore.loadCurrencies().catch(() => {})
}

function retryCatalog() {
  void settingsStore.loadCurrencies(true).catch(() => {})
}
</script>

<template>
  <div class="grid grid-cols-1 content-start gap-1.5" :data-invalid="props.error ? '' : undefined">
    <Label v-if="props.label" :for="triggerId" :required="props.required">{{ props.label }}</Label>

    <ComboboxRoot
      v-model="picked"
      v-model:open="open"
      :disabled="props.disabled"
      :ignore-filter="true"
      open-on-click
      @update:open="onOpen"
    >
      <!-- Reka hardcodes an English `aria-label` on its trigger; it is overridden,
           and the button is the only tab stop while closed. -->
      <ComboboxAnchor as-child class="w-full">
        <ComboboxTrigger as-child :aria-label="props.label || t('settings.currencySelect')">
          <!-- `#trigger`: a control of its own opens the list («+ Валюта» among the favourites). -->
          <slot v-if="$slots.trigger" name="trigger" :open="open" />
          <Button
            v-else
            :id="triggerId"
            variant="outline"
            :aria-expanded="open"
            :aria-invalid="props.error ? true : undefined"
            class="w-full min-w-0 justify-between gap-2 px-3 font-normal data-[state=open]:border-primary"
            :class="[
              props.error && 'border-destructive',
              props.chip && 'h-7 gap-1 rounded-none border-0 border-l bg-transparent pl-2 pr-1 text-foreground shadow-none hover:bg-transparent dark:bg-transparent',
            ]"
          >
            <span v-if="selected" class="flex min-w-0 items-baseline gap-1.5">
              <span v-if="props.chip && currencySymbol(selected) !== selected.toUpperCase()">{{ currencySymbol(selected) }}</span>
              <span :class="props.chip ? 'uppercase' : 'font-medium uppercase'">{{ selected }}</span>
              <span v-if="selectedName && !props.compact && !props.chip" class="truncate text-xs text-muted-foreground">{{ selectedName }}</span>
            </span>
            <span v-else class="truncate text-muted-foreground">
              {{ props.placeholder || t('settings.currencySearchPlaceholder') }}
            </span>
            <ChevronDown class="size-4 shrink-0 text-muted-foreground" />
          </Button>
        </ComboboxTrigger>
      </ComboboxAnchor>

      <!-- Above dialogs (z-50): the picker opens inside forms in dialogs. -->
      <ComboboxList class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-[min(14rem,calc(100vw-2rem))]" align="start">
        <ComboboxInput
          v-model="search"
          auto-focus
          autocomplete="off"
          spellcheck="false"
          :display-value="() => search"
          :placeholder="t('settings.currencySearchPlaceholder')"
        />

        <ComboboxViewport class="max-h-72 p-1">
          <ComboboxVirtualizer
            v-slot="{ option }"
            :options="options"
            :estimate-size="CURRENCY_ROW_HEIGHT"
            :overscan="8"
            :text-content="(row: CurrencyOption) => row.code"
          >
            <!-- A group title: disabled, so the arrows skip it. -->
            <ComboboxItem
              v-if="option.header"
              :value="option.code"
              disabled
              class="h-8 w-full px-2 text-2xs font-medium uppercase tracking-wide text-muted-foreground data-[disabled]:opacity-100"
            >
              {{ option.name }}
            </ComboboxItem>
            <ComboboxItem v-else :value="option.code" class="h-8 w-full cursor-pointer">
              <Check v-if="option.code === picked" class="size-3.5 text-primary" aria-hidden="true" />
              <Star v-else-if="option.favorite" class="size-3.5 fill-warning text-warning" aria-hidden="true" />
              <span v-else class="size-3.5 shrink-0" aria-hidden="true" />
              <span v-if="option.favorite" class="sr-only">{{ t('currency.favoriteMark') }}</span>
              <template v-if="option.code !== NONE">
                <span class="w-14 shrink-0 font-medium uppercase">{{ option.code }}</span>
                <span class="min-w-0 flex-1 truncate text-xs text-muted-foreground">{{ option.name }}</span>
                <span class="shrink-0 text-2xs uppercase text-muted-foreground">
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
      </ComboboxList>
    </ComboboxRoot>

    <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>
    <p v-else-if="catalogNotice" class="text-xs text-muted-foreground">
      {{ catalogNotice }}
      <Button
        variant="link"
        class="h-auto p-0 text-xs"
        :disabled="settingsStore.currenciesLoading"
        @click="retryCatalog"
      >
        {{ t('currency.catalogRetry') }}
      </Button>
    </p>
  </div>
</template>
