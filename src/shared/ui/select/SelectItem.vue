<script setup lang="ts">
import { inject, onBeforeUnmount } from 'vue'
import { SelectItem as RekaSelectItem, SelectItemIndicator, SelectItemText } from 'reka-ui'
import { Check } from 'lucide-vue-next'

import { EMPTY_VALUE, SELECT_EMPTY_ITEMS } from './select.constants'
import type { SelectItemProps } from './select.types'

/** One entry of a `Select`; `value=""` is a real choice (`Any`, «—»). */
const props = defineProps<SelectItemProps>()

// Counted once: an item's value does not change in this app's lists.
const emptyItems = inject(SELECT_EMPTY_ITEMS, null)
const isEmpty = props.value === ''
if (isEmpty && emptyItems) emptyItems.value += 1
onBeforeUnmount(() => {
  if (isEmpty && emptyItems) emptyItems.value -= 1
})
</script>

<template>
  <RekaSelectItem
    data-slot="select-item"
    :value="isEmpty ? EMPTY_VALUE : props.value"
    :disabled="props.disabled"
    class="relative flex w-full cursor-default items-center gap-2 rounded-sm py-1.5 pr-8 pl-2 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
  >
    <span class="absolute right-2 flex size-3.5 items-center justify-center">
      <SelectItemIndicator>
        <Check class="size-4" />
      </SelectItemIndicator>
    </span>
    <SelectItemText>
      <slot />
    </SelectItemText>
  </RekaSelectItem>
</template>
