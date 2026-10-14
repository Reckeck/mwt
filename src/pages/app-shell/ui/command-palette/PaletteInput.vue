<script setup lang="ts">
import { ref } from 'vue'
import { ListboxFilter } from 'reka-ui'
import { Search } from 'lucide-vue-next'

import { Kbd } from '@/shared/ui/kbd'
import { useCommand } from '@/shared/ui/command'

/**
 * The search field in the header. Unlike `CommandInput` it does not grab the
 * focus when the page opens; Ctrl+K (or a click) puts it there.
 */
defineProps<{ placeholder: string; label: string }>()
const emit = defineEmits<{ focus: [] }>()

const { filterState } = useCommand()
const field = ref<InstanceType<typeof ListboxFilter> | null>(null)

defineExpose({
  focus: () => (field.value?.$el as HTMLInputElement | undefined)?.focus(),
  blur: () => (field.value?.$el as HTMLInputElement | undefined)?.blur(),
})
</script>

<template>
  <div
    class="flex h-8 w-full min-w-0 items-center gap-2 rounded-md border border-input bg-background px-2 text-sm shadow-xs focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50"
  >
    <Search class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    <ListboxFilter
      ref="field"
      v-model="filterState.search"
      :placeholder="placeholder"
      :aria-label="label"
      class="h-full min-w-0 flex-1 bg-transparent outline-hidden placeholder:text-muted-foreground"
      @focus="emit('focus')"
      @click="emit('focus')"
    />
    <Kbd class="hidden shrink-0 lg:inline-flex">Ctrl K</Kbd>
  </div>
</template>
