<script setup lang="ts">
import type { HTMLAttributes } from 'vue'
import { cn } from '@/shared/lib/utils'

const props = defineProps<{
  class?: HTMLAttributes['class']
  /** Classes of the scrolling wrapper: a max height makes it the box a sticky header sticks to. */
  containerClass?: HTMLAttributes['class']
}>()
</script>

<template>
  <div data-slot="table-container" :class="cn('relative w-full overflow-auto', props.containerClass)">
    <table data-slot="table" :class="cn('w-full caption-bottom text-sm', props.class)">
      <slot />
    </table>
  </div>
</template>
