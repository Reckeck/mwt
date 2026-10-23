<script setup lang="ts">
import { ref, watch } from 'vue'
import { ListboxFilter } from 'reka-ui'
import { Search } from 'lucide-vue-next'

import { useCommand } from '@/shared/ui/command'

/**
 * The field at the top of Ctrl K (Paths 1 · 7). It reports the query up, so
 * the window can decide on the glance and the extra groups.
 */
defineProps<{ placeholder: string; label: string }>()
const emit = defineEmits<{ search: [value: string] }>()

const { filterState } = useCommand()
const field = ref<InstanceType<typeof ListboxFilter> | null>(null)
watch(() => filterState.search, (value) => emit('search', value), { immediate: true })

defineExpose({
  focus: () => (field.value?.$el as HTMLInputElement | undefined)?.focus(),
})
</script>

<template>
  <div class="flex h-full min-w-0 items-center gap-2 text-sm">
    <Search class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    <ListboxFilter
      ref="field"
      v-model="filterState.search"
      :placeholder="placeholder"
      :aria-label="label"
      class="h-full min-w-0 flex-1 bg-transparent text-base outline-hidden placeholder:text-muted-foreground md:text-sm"
    />
  </div>
</template>
