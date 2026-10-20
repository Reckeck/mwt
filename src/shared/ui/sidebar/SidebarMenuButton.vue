<script setup lang="ts">
import type { Component } from "vue"
import type { SidebarMenuButtonProps } from "./SidebarMenuButtonChild.vue"
import { reactiveOmit } from "@vueuse/core"
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import SidebarMenuButtonChild from "./SidebarMenuButtonChild.vue"
import { useSidebar } from "./utils"

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(defineProps<SidebarMenuButtonProps & {
  tooltip?: string | Component
}>(), {
  as: "button",
  variant: "default",
  size: "default",
})

const { isMobile, state } = useSidebar()

const delegatedProps = reactiveOmit(props, "tooltip")
</script>

<!-- The label tooltip exists only while the sidebar is collapsed to icons: an expanded
     sidebar shows the label itself, and a tooltip per item cost a mount per wallet. -->

<template>
  <SidebarMenuButtonChild v-if="!tooltip || state !== 'collapsed' || isMobile" v-bind="{ ...delegatedProps, ...$attrs }">
    <slot />
  </SidebarMenuButtonChild>

  <!-- Icons only: the name shows at once, as a label would. -->
  <Tooltip v-else :delay-duration="0">
    <TooltipTrigger as-child>
      <SidebarMenuButtonChild v-bind="{ ...delegatedProps, ...$attrs }">
        <slot />
      </SidebarMenuButtonChild>
    </TooltipTrigger>
    <TooltipContent side="right" align="center">
      <template v-if="typeof tooltip === 'string'">
        {{ tooltip }}
      </template>
      <component :is="tooltip" v-else />
    </TooltipContent>
  </Tooltip>
</template>
