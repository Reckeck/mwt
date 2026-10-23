<script setup lang="ts">
import { CategoryGlanceBody, WalletGlanceBody } from '@/widgets/quick-glance'
import { currentMonth } from '@/shared/lib/period'

import type { PaletteHighlight } from './use-palette-highlight'

/**
 * «Быстрый взгляд» beside the results (Paths 2 · 8): the highlighted category
 * or wallet in numbers, without leaving the search.
 */
const props = defineProps<{ item: PaletteHighlight }>()
const emit = defineEmits<{ open: [target: EntityRef] }>()
</script>

<template>
  <div class="flex min-h-0 flex-col gap-2.5 overflow-y-auto p-3.5 text-sm" data-palette-peek>
    <CategoryGlanceBody
      v-if="props.item.target.kind === 'category'"
      :category="props.item.target.id"
      :month="currentMonth()"
      class="flex-1"
      @open="emit('open', props.item.target)"
    />
    <WalletGlanceBody v-else-if="props.item.target.kind === 'store'" :store-id="props.item.target.id" :name="props.item.title" />
  </div>
</template>
