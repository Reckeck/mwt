<script setup lang="ts">
import { computed } from 'vue'

import { Button } from '@/shared/ui/button'

/** One key of a JSON answer: a leaf picks its path, an object or array opens below. */
const props = defineProps<{ name: string; value: unknown; path: string; selected: string; depth: number }>()
const emit = defineEmits<{ pick: [path: string, isArray: boolean] }>()

const children = computed<[string, unknown][]>(() => {
  if (Array.isArray(props.value)) return props.value.slice(0, 50).map((item, index) => [String(index), item])
  if (props.value && typeof props.value === 'object') return Object.entries(props.value).slice(0, 100)
  return []
})
const isArray = computed(() => Array.isArray(props.value))
const leaf = computed(() => !children.value.length)
const childPath = (key: string) => (props.path ? `${props.path}.${key}` : key)
const preview = computed(() => {
  if (isArray.value) return `[${(props.value as unknown[]).length}]`
  if (!leaf.value) return '{…}'
  return JSON.stringify(props.value)
})
</script>

<template>
  <div>
    <Button
      variant="ghost"
      size="sm"
      class="h-auto w-full min-w-0 justify-start gap-2 px-1 py-0.5 text-left text-xs font-normal"
      :class="props.selected === props.path && 'bg-accent text-accent-foreground'"
      :style="{ paddingLeft: `${props.depth * 12 + 4}px` }"
      @click="emit('pick', props.path, isArray)"
    >
      <span class="shrink-0 font-medium">{{ props.name }}</span>
      <span class="truncate font-mono text-muted-foreground">{{ preview }}</span>
    </Button>
    <template v-if="!leaf && props.depth < 6">
      <JsonNode
        v-for="[key, child] in children"
        :key="key"
        :name="key"
        :value="child"
        :path="childPath(key)"
        :selected="props.selected"
        :depth="props.depth + 1"
        @pick="(path, array) => emit('pick', path, array)"
      />
    </template>
  </div>
</template>
