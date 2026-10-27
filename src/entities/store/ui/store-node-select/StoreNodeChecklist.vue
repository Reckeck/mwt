<script setup lang="ts">
import { useId } from 'vue'

import { Checkbox } from '@/shared/ui/checkbox'

/** A checkbox per node, by its path, with «all» (`emptyLabel`) on top: the body of the multi-select, or a filter panel's own section. */
const props = defineProps<{ options: { id: string; label: string }[]; emptyLabel?: string }>()
const ids = defineModel<string[]>({ required: true })

const uid = useId()

function toggle(id: string, on: boolean) {
  ids.value = on ? [...ids.value, id] : ids.value.filter((entry) => entry !== id)
}
</script>

<template>
  <label
    v-if="props.emptyLabel"
    :for="`${uid}-all`"
    class="touch-hit flex min-w-0 cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
  >
    <Checkbox :id="`${uid}-all`" :model-value="!ids.length" @update:model-value="ids = []" />
    {{ props.emptyLabel }}
  </label>
  <label
    v-for="option in props.options"
    :key="option.id"
    :for="`${uid}-${option.id}`"
    class="touch-hit flex min-w-0 cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
  >
    <Checkbox :id="`${uid}-${option.id}`" :model-value="ids.includes(option.id)" @update:model-value="(on) => toggle(option.id, on === true)" />
    <span class="truncate">{{ option.label }}</span>
  </label>
</template>
