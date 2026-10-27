<script setup lang="ts">
import { ref, watch } from 'vue'
import { ChevronDown } from 'lucide-vue-next'

import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { cn } from '@/shared/lib/utils'

import { AssetInput } from '../asset-input'
import type { AssetChipProps } from './asset-chip.types'

/**
 * The asset as a chip inside an amount field (OpForm-1 · 1): «RUB ▾». The
 * wallet already picked it, so it stays one tap away instead of a field of
 * its own; the tap opens the usual search in a popover.
 */
const props = withDefaults(defineProps<AssetChipProps>(), { placeholder: '—', invalid: false })
const asset = defineModel<AssetRef | null>({ default: null })

const open = ref(false)
// A pick closes the popover: the chip shows it right away.
watch(asset, (value) => {
  if (value) open.value = false
})
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger
      type="button"
      :aria-label="props.label"
      :class="
        cn(
          'touch-hit flex h-7 items-center gap-1 rounded-md bg-secondary px-2 text-xs font-semibold text-secondary-foreground',
          'transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
          props.invalid && 'text-destructive',
        )
      "
      data-asset-chip
    >
      {{ asset?.symbol?.toUpperCase() ?? props.placeholder }}
      <ChevronDown class="size-3.5" />
    </PopoverTrigger>
    <PopoverContent overlay="asset-chip" align="end" class="w-[min(20rem,calc(100vw-2rem))] p-2">
      <AssetInput v-model="asset" label="" open-on-focus />
    </PopoverContent>
  </Popover>
</template>
