<script setup lang="ts">
import { DropdownMenuItem } from 'reka-ui'

import type { DropdownMenuItemProps } from './dropdown-menu-item.types'

defineProps<DropdownMenuItemProps>()

const emit = defineEmits<{ select: [string] }>()
</script>

<template>
  <DropdownMenuItem
    class="flex cursor-pointer select-none items-center gap-2 rounded px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground"
    :class="destructive && 'text-destructive data-[highlighted]:text-destructive'"
    @select="emit('select', value)"
  >
    <slot />
  </DropdownMenuItem>
</template>
