<script setup lang="ts">
import type { HTMLAttributes, Ref } from "vue"
import { useEventListener, useMediaQuery, useVModel } from "@vueuse/core"
import { matchesHotkey } from '@/shared/lib/hotkeys'
import { PHONE_QUERY } from '@/shared/lib/phone'
import { computed, ref, watch } from "vue"
import { cn } from '@/shared/lib/utils'
import { provideSidebarContext, readSidebarOpen, NARROW_QUERY, SIDEBAR_WIDTH, SIDEBAR_WIDTH_ICON, writeSidebarOpen } from "./utils"

const props = withDefaults(defineProps<{
  defaultOpen?: boolean
  open?: boolean
  class?: HTMLAttributes["class"]
}>(), {
  defaultOpen: () => readSidebarOpen(),
  open: undefined,
})

const emits = defineEmits<{
  "update:open": [open: boolean]
}>()

const isMobile = useMediaQuery(PHONE_QUERY)
const openMobile = ref(false)

const open = useVModel(props, "open", emits, {
  defaultValue: props.defaultOpen ?? false,
  passive: (props.open === undefined) as false,
}) as Ref<boolean>

function setOpen(value: boolean) {
  open.value = value // emits('update:open', value)

  // A desktop webview has no use for a cookie; remember the state locally.
  if (!narrow.value) writeSidebarOpen(open.value)
}

function setOpenMobile(value: boolean) {
  openMobile.value = value
}

// Helper to toggle the sidebar.
function toggleSidebar() {
  return isMobile.value ? setOpenMobile(!openMobile.value) : setOpen(!open.value)
}

useEventListener("keydown", (event: KeyboardEvent) => {
  if (matchesHotkey(event, 'sidebar')) {
    event.preventDefault()
    toggleSidebar()
  }
})

// A narrow window folds the sidebar to icons by itself; widening it again brings back
// what the user chose. The fold is not remembered, only the user's own Ctrl B.
const narrow = useMediaQuery(NARROW_QUERY)
watch(narrow, (isNarrow) => {
  open.value = isNarrow ? false : readSidebarOpen()
}, { immediate: true })

// We add a state so that we can do data-state="expanded" or "collapsed".
// This makes it easier to style the sidebar with Tailwind classes.
const state = computed(() => open.value ? "expanded" : "collapsed")

provideSidebarContext({
  state,
  open,
  setOpen,
  isMobile,
  narrow,
  openMobile,
  setOpenMobile,
  toggleSidebar,
})
</script>

<template>
  <div
    data-slot="sidebar-wrapper"
    :style="{
      '--sidebar-width': SIDEBAR_WIDTH,
      '--sidebar-width-icon': SIDEBAR_WIDTH_ICON,
    }"
    :class="cn('group/sidebar-wrapper has-data-[variant=inset]:bg-sidebar flex min-h-svh w-full', props.class)"
    v-bind="$attrs"
  >
    <slot />
  </div>
</template>
