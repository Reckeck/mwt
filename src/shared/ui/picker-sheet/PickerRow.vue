<script setup lang="ts">
import { Check, ChevronRight } from 'lucide-vue-next'

/**
 * One choice of a Picker (board 1097003): 40 px on a desktop, 48 on a phone; an icon circle
 * (`icon` slot, 28/32 px), the title 14/20 medium with an optional subtitle, an optional amount
 * on the right with a `≈ base` line under it, and a check when picked. `chevron` marks a row that
 * opens its own list, `indent` a row nested under the one above. Hover and keyboard focus tint it.
 */
const props = defineProps<{
  title: string
  subtitle?: string
  amount?: string
  amountHint?: string
  selected?: boolean
  chevron?: boolean
  indent?: boolean
}>()
defineEmits<{ click: [] }>()
</script>

<template>
  <button
    type="button"
    class="flex min-h-12 w-full items-center gap-2.5 rounded-sm px-2 text-left outline-none hover:bg-hover focus-visible:bg-hover md:min-h-10"
    :class="[props.selected && 'bg-selected', props.indent && 'pl-9']"
    :aria-pressed="props.selected"
    @click="$emit('click')"
  >
    <span
      v-if="$slots.icon"
      class="grid size-8 shrink-0 place-items-center rounded-full bg-muted text-foreground md:size-7 [&_svg]:size-5"
    >
      <slot name="icon" />
    </span>
    <span class="grid min-w-0 flex-1 py-1">
      <span class="truncate text-sm font-medium">{{ props.title }}</span>
      <span v-if="props.subtitle" class="truncate text-xs text-muted-foreground">{{ props.subtitle }}</span>
    </span>
    <!-- money: plain — the caller passes the balance and its base preformatted -->
    <span v-if="props.amount" class="grid shrink-0 text-right tabular-nums">
      <span class="text-sm font-semibold">{{ props.amount }}</span>
      <span v-if="props.amountHint" class="text-xs text-muted-foreground">{{ props.amountHint }}</span>
    </span>
    <ChevronRight v-if="props.chevron" class="size-[18px] shrink-0 text-tertiary" aria-hidden="true" />
    <Check v-if="props.selected" class="size-4 shrink-0" aria-hidden="true" />
  </button>
</template>
