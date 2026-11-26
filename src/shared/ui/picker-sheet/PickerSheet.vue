<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Search, X } from 'lucide-vue-next'

import { AdaptivePanel } from '@/shared/ui/adaptive-panel'

/**
 * Choosing one item (Picker, board 1097003): a borderless search row over a hairline, then the
 * list in the default slot (`PickerSection`, `PickerRow`). A bottom sheet on a phone (boards
 * 532847, 532506), a popover under the trigger on a desktop, as wide as the trigger between
 * 320 and 420 px. `searchable: false` drops the search row (a short list). The trigger goes in
 * the `trigger` slot.
 */
const props = withDefaults(defineProps<{ title: string; searchLabel: string; overlay: string; searchable?: boolean }>(), {
  searchable: true,
})
const open = defineModel<boolean>('open', { default: false })
const search = defineModel<string>('search', { default: '' })
const { t } = useI18n()
</script>

<template>
  <AdaptivePanel
    v-model:open="open"
    surface="picker"
    :title="props.title"
    :overlay="props.overlay"
    width="w-(--reka-popover-trigger-width) min-w-80 max-w-[420px]"
  >
    <template #trigger><slot name="trigger" /></template>
    <template v-if="props.searchable" #header>
      <div class="flex h-11 shrink-0 items-center gap-2 border-b border-border px-2.5 md:h-9">
        <Search class="size-4 shrink-0 text-tertiary" aria-hidden="true" />
        <input
          v-model="search"
          type="search"
          :aria-label="props.searchLabel"
          :placeholder="props.searchLabel"
          autocomplete="off"
          class="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-tertiary [&::-webkit-search-cancel-button]:hidden"
        />
        <button
          type="button"
          class="grid size-5 shrink-0 place-items-center rounded-full text-tertiary hover:text-foreground"
          :class="!search && 'invisible'"
          :aria-label="t('ui.clear')"
          @click="search = ''"
        >
          <X class="size-3.5" aria-hidden="true" />
        </button>
      </div>
    </template>
    <slot />
  </AdaptivePanel>
</template>
