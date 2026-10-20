<script setup lang="ts">
import { computed, provide, ref, useAttrs } from 'vue'
import { SelectContent, SelectIcon, SelectPortal, SelectRoot, SelectTrigger, SelectValue, SelectViewport } from 'reka-ui'
import { ChevronDown } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'

import { EMPTY_VALUE, SELECT_EMPTY_ITEMS } from './select.constants'
import type { SelectProps } from './select.types'

/**
 * One choice from a short list: a field like an input that opens a list
 * under it, the same on every system (a native `<select>` drops a list styled
 * by the OS). Items are `SelectItem` children, as `<option>` were. `id`,
 * `aria-*` and `class` go to the field. Type-ahead and arrows work as in a
 * native select.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<SelectProps>(), { placeholder: undefined, contentWidth: 'trigger' })
const model = defineModel<string>({ default: '' })

const attrs = useAttrs()
const emptyItems = ref(0)
provide(SELECT_EMPTY_ITEMS, emptyItems)

const inner = computed(() => {
  const value = model.value ?? ''
  if (value !== '') return value
  return emptyItems.value > 0 ? EMPTY_VALUE : undefined
})

function pick(next: unknown) {
  if (typeof next !== 'string') return
  model.value = next === EMPTY_VALUE ? '' : next
}
</script>

<template>
  <SelectRoot :model-value="inner" :disabled="props.disabled" @update:model-value="pick">
    <SelectTrigger
      data-slot="select-trigger"
      v-bind="attrs"
      :aria-invalid="props.invalid || (attrs['aria-invalid'] as boolean | undefined) || undefined"
      :class="
        cn(
          'touch-hit flex h-9 w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-1 text-left text-sm whitespace-nowrap shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 data-[placeholder]:text-muted-foreground aria-invalid:border-destructive [&>span]:truncate',
          props.invalid && 'border-destructive',
          attrs.class as string,
        )
      "
    >
      <SelectValue :placeholder="props.placeholder ?? ''" />
      <SelectIcon as-child>
        <ChevronDown class="size-4 shrink-0 opacity-50" />
      </SelectIcon>
    </SelectTrigger>
    <SelectPortal>
      <SelectContent
        data-slot="select-content"
        position="popper"
        :side-offset="4"
        :class="
          cn(
            'relative z-50 max-h-(--reka-select-content-available-height) overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=top]:slide-in-from-bottom-2',
            props.contentWidth === 'trigger' ? 'min-w-(--reka-select-trigger-width)' : 'min-w-20',
          )
        "
      >
        <SelectViewport class="max-h-72 scroll-py-1 p-1">
          <slot />
        </SelectViewport>
      </SelectContent>
    </SelectPortal>
  </SelectRoot>
</template>
