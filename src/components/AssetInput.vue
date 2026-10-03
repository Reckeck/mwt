<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { CHAIN_LABELS } from '@/lib/chains'
import { call } from '@/lib/ipc'

const props = withDefaults(
  defineProps<{
    /** Label rendered under the input; pass `''` to hide. */
    label?: string
    placeholder?: string
    invalid?: boolean
  }>(),
  { label: '', placeholder: '', invalid: false },
)

const asset = defineModel<AssetRef | null>({ default: null })

const { t } = useI18n()
const query = ref('')
const hits = ref<AssetSearchHit[]>([])
  const open = ref(false)
const loading = ref(false)

let timer: ReturnType<typeof setTimeout> | undefined
let seq = 0

watch(asset, (v) => {
  query.value = v ? v.symbol : ''
})

async function search(q: string) {
  const token = ++seq
  if (q.trim().length < 1) {
    hits.value = []
    loading.value = false
    return
  }
  loading.value = true
  try {
    const res = await call<AssetSearchHit[]>('search_assets', { query: q.trim() })
    if (token === seq) hits.value = res
  } catch {
    if (token === seq) hits.value = []
  } finally {
    if (token === seq) loading.value = false
  }
}

function onInput(e: Event) {
  const value = (e.target as HTMLInputElement).value
  query.value = value
  open.value = true
  asset.value = null
  clearTimeout(timer)
  timer = setTimeout(() => void search(value), 250)
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
  query.value = symbol
  open.value = false
  hits.value = []
}

/** Free-text fallback: treat an unknown ticker as a plain fiat unit. */
function pickFiat() {
  const symbol = query.value.trim().toUpperCase()
  if (!symbol) return
  asset.value = {
    symbol,
    name: symbol,
    coingecko_id: null,
    chain: 'fiat',
    contract: null,
    quote: symbol,
  }
  open.value = false
  hits.value = []
}

function onBlur() {
  // let the click on a dropdown row land first
  setTimeout(() => {
    open.value = false
  }, 120)
}

const canOfferFiat = () =>
  query.value.trim().length > 0 &&
  !hits.value.some((h) => h.symbol.toUpperCase() === query.value.trim().toUpperCase())

const subtitle = (a: AssetRef) =>
  a.chain && a.chain !== 'fiat' ? `${a.name} · ${CHAIN_LABELS[a.chain]}` : a.name
</script>

<template>
  <div class="relative grid gap-1.5">
    <label v-if="props.label" class="text-sm font-medium leading-none">{{ props.label }}</label>
    <div class="relative">
      <input
        :value="query"
        type="text"
        autocomplete="off"
        spellcheck="false"
        :placeholder="props.placeholder || t('asset.searchPlaceholder')"
        class="h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        :class="props.invalid && 'border-destructive'"
        @input="onInput"
        @focus="open = true"
        @blur="onBlur"
      >
      <span v-if="loading" class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs text-muted-foreground">
        …
      </span>
    </div>

    <p v-if="asset" class="truncate text-xs text-muted-foreground">{{ subtitle(asset) }}</p>

    <ul
      v-if="open && (hits.length > 0 || canOfferFiat())"
      class="absolute z-30 mt-1 max-h-64 w-full overflow-y-auto rounded-md border border-border bg-popover p-1 text-popover-foreground shadow-lg"
    >
      <li v-for="h in hits" :key="`${h.symbol}-${h.contract ?? ''}`">
        <button
          type="button"
          class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent hover:text-accent-foreground"
          @mousedown.prevent="pick(h)"
        >
          <span class="w-20 shrink-0 truncate font-medium">{{ h.symbol.toUpperCase() }}</span>
          <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ h.name }}</span>
          <span v-if="h.chain && h.chain !== 'fiat'" class="shrink-0 text-xs text-muted-foreground">
            {{ CHAIN_LABELS[h.chain] }}
          </span>
        </button>
      </li>
      <li v-if="canOfferFiat()">
        <button
          type="button"
          class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent hover:text-accent-foreground"
          @mousedown.prevent="pickFiat"
        >
          <span class="shrink-0 font-medium">{{ query.trim().toUpperCase() }}</span>
          <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ t('asset.useAsFiat') }}</span>
        </button>
      </li>
    </ul>
  </div>
</template>
