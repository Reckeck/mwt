<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ComboboxInput } from 'reka-ui'

import { useCurrencyOptions } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { fiatRef } from '@/shared/lib/asset-ref'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'
import { ComboboxAnchor, ComboboxGroup, ComboboxItem, ComboboxList, ComboboxRoot, ComboboxViewport } from '@/shared/ui/combobox'
import { Label } from '@/shared/ui/label'

import type { AssetInputProps } from './asset-input.types'

/** Fiat rows shown at most; the full catalog is one more letter away. */
const FIAT_LIMIT = 6

/**
 * Search any asset: fiat from the currency catalog, crypto from the backend
 * search, or the typed code as a fiat unit. A text field with a combobox list
 * (shadcn-vue parts): arrows move, Enter picks, Esc closes.
 */
const props = withDefaults(defineProps<AssetInputProps>(), {
  label: '',
  placeholder: '',
  invalid: false,
  required: false,
  error: undefined,
})

const asset = defineModel<AssetRef | null>({ default: null })

const { t } = useI18n()
const settings = useSettingsStore()
const inputId = useId()
const query = ref('')
const hits = ref<AssetSearchHit[]>([])
const open = ref(false)
const loading = ref(false)

let timer: ReturnType<typeof setTimeout> | undefined
let seq = 0
/** Typing clears the model; that clear must not wipe what is being typed. */
let clearedByTyping = false

// Immediate: an asset handed in up front (an edit, a default) shows right away.
watch(
  asset,
  (value) => {
    if (value === null && clearedByTyping) {
      clearedByTyping = false
      return
    }
    clearedByTyping = false
    query.value = value ? value.symbol : ''
  },
  { immediate: true },
)

/**
 * Fiat comes from the currency catalog, not from CoinGecko: a coin that happens
 * to be called USD or RUB is not the dollar, and has no FX rate, so fees and
 * losses in it could not be counted.
 */
const { filter: filterFiat } = useCurrencyOptions(['fiat'], [])
const fiatHits = computed(() => filterFiat(query.value).slice(0, FIAT_LIMIT))

async function search(rawQuery: string) {
  const token = ++seq
  if (rawQuery.trim().length < 1) {
    hits.value = []
    loading.value = false
    return
  }
  loading.value = true
  try {
    const res = await call<AssetSearchHit[]>('search_assets', { query: rawQuery.trim() })
    if (token === seq) hits.value = res
  } catch {
    if (token === seq) hits.value = []
  } finally {
    if (token === seq) loading.value = false
  }
}

function onFocus() {
  open.value = true
  if (!settings.currencies) void settings.loadCurrencies().catch(() => {})
}

function onInput(value: string) {
  query.value = value
  open.value = true
  if (asset.value !== null) {
    clearedByTyping = true
    asset.value = null
  }
  clearTimeout(timer)
  timer = setTimeout(() => void search(value), 250)
}

function close(symbol: string) {
  query.value = symbol
  open.value = false
  hits.value = []
}

function pick(hit: AssetSearchHit) {
  const symbol = hit.symbol.toUpperCase()
  asset.value = {
    symbol,
    name: hit.name || symbol,
    coingecko_id: hit.coingecko_id,
    chain: hit.chain,
    contract: hit.contract,
    quote: symbol,
  }
  close(symbol)
}

/** A catalog currency, or free text the catalog does not know, as a fiat unit. */
function pickFiat(code: string) {
  const currency = fiatRef(code)
  if (!currency.symbol) return
  asset.value = currency
  close(currency.symbol)
}

/** Rows are keyed by kind: `fiat:USD`, `hit:3`, `typed`; picking one runs its action. */
function onPick(key: unknown) {
  if (typeof key !== 'string') return
  if (key.startsWith('fiat:')) pickFiat(key.slice(5))
  else if (key.startsWith('hit:')) {
    const hit = hits.value[Number(key.slice(4))]
    if (hit) pick(hit)
  } else if (key === 'typed') pickFiat(query.value)
}

/** Free text is offered only when neither the catalog nor the search knows the code. */
const typed = computed(() => query.value.trim().toUpperCase())
const canOfferFiat = computed(
  () =>
    typed.value.length > 0 &&
    !fiatHits.value.some((row) => row.code.toUpperCase() === typed.value) &&
    !hits.value.some((hit) => hit.symbol.toUpperCase() === typed.value),
)
const hasRows = computed(() => fiatHits.value.length > 0 || hits.value.length > 0 || canOfferFiat.value)

const subtitle = (value: AssetRef) => {
  if (value.chain === 'fiat') {
    const name = filterFiat(value.symbol).find((row) => row.code.toUpperCase() === value.symbol)?.name
    return name ? `${name} · ${t('currency.kind.fiat')}` : t('currency.kind.fiat')
  }
  return value.chain ? `${value.name} · ${CHAIN_LABELS[value.chain]}` : value.name
}
</script>

<template>
  <div class="grid content-start gap-1.5" :data-invalid="props.error ? '' : undefined">
    <Label v-if="props.label" :for="inputId" :required="props.required">{{ props.label }}</Label>
    <ComboboxRoot
      v-model:open="open"
      :model-value="undefined"
      :ignore-filter="true"
      :reset-search-term-on-blur="false"
      :reset-search-term-on-select="false"
      @update:model-value="onPick"
    >
      <ComboboxAnchor class="relative w-full">
        <ComboboxInput
          :id="inputId"
          :model-value="query"
          :display-value="() => query"
          autocomplete="off"
          spellcheck="false"
          :aria-invalid="props.invalid || props.error ? true : undefined"
          :placeholder="props.placeholder || t('asset.searchPlaceholder')"
          :class="
            cn(
              'h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
              (props.invalid || props.error) && 'border-destructive',
            )
          "
          @update:model-value="onInput"
          @focus="onFocus"
        />
        <span v-if="loading" class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs text-muted-foreground">
          …
        </span>
      </ComboboxAnchor>

      <ComboboxList
        v-if="hasRows"
        class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-64"
        align="start"
        @open-auto-focus.prevent
      >
        <ComboboxViewport class="max-h-72 p-1">
          <ComboboxGroup v-if="fiatHits.length" :heading="t('asset.group.fiat')">
            <ComboboxItem v-for="row in fiatHits" :key="`fiat-${row.code}`" :value="`fiat:${row.code}`">
              <span class="w-20 shrink-0 truncate font-medium uppercase">{{ row.code }}</span>
              <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ row.name }}</span>
            </ComboboxItem>
          </ComboboxGroup>
          <ComboboxGroup v-if="hits.length" :heading="t('asset.group.crypto')">
            <ComboboxItem
              v-for="(hit, index) in hits"
              :key="`${hit.coingecko_id ?? hit.symbol}-${hit.contract ?? ''}`"
              :value="`hit:${index}`"
            >
              <span class="w-20 shrink-0 truncate font-medium">{{ hit.symbol.toUpperCase() }}</span>
              <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ hit.name }}</span>
              <span v-if="hit.chain && hit.chain !== 'fiat'" class="shrink-0 text-xs text-muted-foreground">
                {{ CHAIN_LABELS[hit.chain] }}
              </span>
            </ComboboxItem>
          </ComboboxGroup>
          <ComboboxItem v-if="canOfferFiat" value="typed">
            <span class="shrink-0 font-medium">{{ typed }}</span>
            <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ t('asset.useAsFiat') }}</span>
          </ComboboxItem>
        </ComboboxViewport>
      </ComboboxList>
    </ComboboxRoot>

    <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>
    <p v-else-if="asset" class="truncate text-xs text-muted-foreground">{{ subtitle(asset) }}</p>
  </div>
</template>
