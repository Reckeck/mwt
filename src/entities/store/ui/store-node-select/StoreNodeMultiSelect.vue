<script setup lang="ts">
import { computed, useAttrs, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { Checkbox } from '@/shared/ui/checkbox'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

/** The list behind `StoreNodeSelect multiple`: a checkbox per node, by its path. */
const props = defineProps<{ options: { id: string; label: string }[]; emptyLabel?: string; placeholder?: string }>()
const ids = defineModel<string[]>({ required: true })

defineOptions({ inheritAttrs: false })

const { t } = useI18n()
/** The caller's class wins over the trigger's own width and height. */
const attrs = useAttrs()
const uid = useId()

const summary = computed(() => {
  const picked = props.options.filter((option) => ids.value.includes(option.id))
  if (!picked.length) return props.emptyLabel ?? props.placeholder ?? ''
  if (picked.length === 1) return picked[0].label
  return t('store.pickedCount', { count: picked.length })
})

function toggle(id: string, on: boolean) {
  ids.value = on ? [...ids.value, id] : ids.value.filter((entry) => entry !== id)
}
</script>

<template>
  <Popover>
    <PopoverTrigger
      v-bind="$attrs"
      :class="
        cn(
          'flex h-9 w-full items-center justify-between gap-2 rounded-md border border-input bg-transparent px-3 py-1 text-left text-sm whitespace-nowrap shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
          !ids.length && 'text-muted-foreground',
          attrs.class as string | undefined,
        )
      "
    >
      <span class="truncate">{{ summary }}</span>
      <ChevronDown class="size-4 shrink-0 opacity-50" />
    </PopoverTrigger>
    <PopoverContent overlay="store-multiselect" align="start" class="max-h-80 w-(--reka-popover-trigger-width) min-w-[min(14rem,calc(100vw-2rem))] overflow-y-auto p-1">
      <label
        v-if="props.emptyLabel"
        :for="`${uid}-all`"
        class="touch-hit flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
      >
        <Checkbox :id="`${uid}-all`" :model-value="!ids.length" @update:model-value="ids = []" />
        {{ props.emptyLabel }}
      </label>
      <label
        v-for="option in props.options"
        :key="option.id"
        :for="`${uid}-${option.id}`"
        class="touch-hit flex cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
      >
        <Checkbox
          :id="`${uid}-${option.id}`"
          :model-value="ids.includes(option.id)"
          @update:model-value="(on) => toggle(option.id, on === true)"
        />
        <span class="truncate">{{ option.label }}</span>
      </label>
    </PopoverContent>
  </Popover>
</template>
