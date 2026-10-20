<script setup lang="ts">
import { computed, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { ComboboxInput } from 'reka-ui'

import { cn } from '@/shared/lib/utils'
import { parsePairSymbol } from '@/shared/lib/pair'
import { ComboboxAnchor, ComboboxGroup, ComboboxItem, ComboboxList, ComboboxRoot, ComboboxViewport } from '@/shared/ui/combobox'
import { Label } from '@/shared/ui/label'

import { useAssetSearch } from '../asset-input/model/use-asset-search'
import type { PairInputProps } from './pair-input.types'

/** Base assets offered at most; each is shown against every quote. */
const BASE_LIMIT = 5

/**
 * A trading pair (`BTCUSDT`): type a coin and pick it against a quote, or keep
 * the pair as typed. The model is the pair as the exchange spells it.
 */
const props = withDefaults(defineProps<PairInputProps>(), {
  label: '',
  placeholder: '',
  required: false,
  error: undefined,
  id: undefined,
  quotes: () => ['USDT', 'USDC'],
  kind: 'spot',
})
const pair = defineModel<string>({ required: true })

const { t } = useI18n()
const fallbackId = useId()
const inputId = computed(() => props.id ?? fallbackId)
const { hits, loading, schedule, clear } = useAssetSearch()

const rows = computed(() => {
  const seen = new Set<string>()
  return hits.value
    .filter((hit) => {
      const symbol = hit.symbol.toUpperCase()
      if (seen.has(symbol)) return false
      seen.add(symbol)
      return true
    })
    .slice(0, BASE_LIMIT)
    .flatMap((hit) =>
      props.quotes
        .filter((quote) => quote !== hit.symbol.toUpperCase())
        .map((quote) => ({ value: `${hit.symbol.toUpperCase()}${quote}`, base: hit.symbol.toUpperCase(), quote, name: hit.name })),
    )
})
/** The typed text as a pair of its own, when no row says the same. */
const typed = computed(() => pair.value.trim().toUpperCase().replace(/[/\-_: ]+/g, ''))
const offerTyped = computed(() => typed.value.length > 0 && !rows.value.some((row) => row.value === typed.value))

function onInput(value: string) {
  pair.value = value.toUpperCase()
  const query = (parsePairSymbol(value)?.base ?? value).trim()
  if (query) schedule(query)
  else clear()
}

function onPick(value: unknown) {
  if (typeof value !== 'string') return
  pair.value = value
  clear()
}
</script>

<template>
  <div class="grid content-start gap-1.5" :data-invalid="props.error ? '' : undefined">
    <Label v-if="props.label" :for="inputId" :required="props.required">{{ props.label }}</Label>
    <ComboboxRoot
      :model-value="undefined"
      :ignore-filter="true"
      :reset-search-term-on-blur="false"
      :reset-search-term-on-select="false"
      @update:model-value="onPick"
    >
      <ComboboxAnchor class="relative w-full">
        <ComboboxInput
          :id="inputId"
          :model-value="pair"
          :display-value="() => pair"
          autocomplete="off"
          spellcheck="false"
          :aria-invalid="props.error ? true : undefined"
          :placeholder="props.placeholder || t('pair.searchPlaceholder')"
          :class="
            cn(
              'h-9 w-full rounded-md border border-input bg-transparent px-3 text-sm uppercase shadow-sm placeholder:normal-case placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
              props.error && 'border-destructive',
            )
          "
          @update:model-value="onInput"
        />
        <span v-if="loading" class="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs text-muted-foreground">
          …
        </span>
      </ComboboxAnchor>

      <ComboboxList
        v-if="rows.length || offerTyped"
        class="z-[60] w-[var(--reka-combobox-trigger-width)] min-w-[min(16rem,calc(100vw-2rem))]"
        align="start"
        @open-auto-focus.prevent
      >
        <ComboboxViewport class="max-h-72 p-1">
          <ComboboxGroup v-if="rows.length" :heading="t('pair.quote')">
            <ComboboxItem v-for="row in rows" :key="row.value" :value="row.value">
              <span class="w-24 shrink-0 font-medium">{{ row.base }}/{{ row.quote }}</span>
              <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ row.name }}</span>
              <span v-if="props.kind === 'futures'" class="shrink-0 text-2xs uppercase text-muted-foreground">
                {{ t('pair.perpetual') }}
              </span>
            </ComboboxItem>
          </ComboboxGroup>
          <ComboboxItem v-if="offerTyped" :value="typed">
            <span class="shrink-0 font-medium">{{ typed }}</span>
            <span class="min-w-0 flex-1 truncate text-muted-foreground">{{ t('pair.typed') }}</span>
          </ComboboxItem>
        </ComboboxViewport>
      </ComboboxList>
    </ComboboxRoot>
    <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>
  </div>
</template>
