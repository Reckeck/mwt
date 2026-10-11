<script setup lang="ts">
import { DropdownMenuItem } from 'reka-ui'

import { MENU_ITEM_CLASS, MENU_ITEM_DESTRUCTIVE_CLASS } from '@/shared/ui/context-menu'
import { cn } from '@/shared/lib/utils'

import type { DropdownMenuItemProps } from './dropdown-menu-item.types'

defineProps<DropdownMenuItemProps>()

const emit = defineEmits<{ select: [string] }>()
</script>

<template>
  <!-- Same rows as the right-click menu: dangerous ones red, icon included. -->
  <DropdownMenuItem
    :disabled="disabled"
    :class="cn(MENU_ITEM_CLASS, destructive && MENU_ITEM_DESTRUCTIVE_CLASS)"
    @select="emit('select', value)"
  >
    <slot />
  </DropdownMenuItem>
</template>
