<script setup lang="ts">
import { ContextMenuContent, ContextMenuPortal, ContextMenuRoot, ContextMenuTrigger } from 'reka-ui'

import { MENU_CONTENT_CLASS, visibleActions } from './context-menu.constants'
import type { ContextMenuProps } from './context-menu.types'
import ContextMenuEntries from './ContextMenuEntries.vue'

const props = withDefaults(defineProps<ContextMenuProps>(), { disabled: false, lazy: false })
</script>

<template>
  <!-- Right click or Shift+F10 / the Menu key on the focused element opens it. -->
  <ContextMenuRoot :modal="false">
    <ContextMenuTrigger as-child :disabled="props.disabled || (!props.lazy && !visibleActions(props.actions).length)">
      <slot />
    </ContextMenuTrigger>
    <ContextMenuPortal>
      <ContextMenuContent :class="MENU_CONTENT_CLASS" data-context-menu>
        <ContextMenuEntries :actions="props.actions" />
      </ContextMenuContent>
    </ContextMenuPortal>
  </ContextMenuRoot>
</template>
