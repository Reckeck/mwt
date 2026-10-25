<script setup lang="ts">
import type { DialogContentEmits, DialogContentProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import { X } from "lucide-vue-next"
import { reactiveOmit } from "@vueuse/core"
import {
  DialogClose,
  DialogContent,
  DialogPortal,
  useForwardPropsEmits,
} from "reka-ui"
import { useI18n } from 'vue-i18n'
import { cn } from '@/shared/lib/utils'
import SheetOverlay from "./SheetOverlay.vue"
import { keepOpenOnToast } from '@/shared/lib/toast-outside'

interface SheetContentProps extends DialogContentProps {
  class?: HTMLAttributes["class"]
  side?: "top" | "right" | "bottom" | "left"
  /** The screen draws its own way out («‹» in its header). */
  hideClose?: boolean
  /** Name of the layer for `data-overlay`. */
  overlay?: string
}

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(defineProps<SheetContentProps>(), {
  side: "right",
})
const emits = defineEmits<DialogContentEmits>()

const delegatedProps = reactiveOmit(props, "class", "side", "hideClose", "overlay")

const forwarded = useForwardPropsEmits(delegatedProps, emits)
const { t } = useI18n()
</script>

<template>
  <DialogPortal>
    <SheetOverlay />
    <DialogContent
      data-slot="sheet-content"
      :data-overlay="props.overlay"
      @pointer-down-outside="keepOpenOnToast"
      @interact-outside="keepOpenOnToast"
      :class="cn(
        'bg-background data-[state=open]:animate-in data-[state=closed]:animate-out fixed z-50 flex flex-col gap-4 shadow-lg transition ease-in-out data-[state=closed]:duration-150 data-[state=open]:duration-200',
        side === 'right'
          && 'data-[state=closed]:slide-out-to-right data-[state=open]:slide-in-from-right inset-y-0 right-0 h-full w-full border-l pt-safe pb-safe sm:w-3/4 sm:max-w-md',
        side === 'left'
          && 'data-[state=closed]:slide-out-to-left data-[state=open]:slide-in-from-left inset-y-0 left-0 h-full w-3/4 border-r pt-safe pb-safe sm:max-w-md',
        side === 'top'
          && 'data-[state=closed]:slide-out-to-top data-[state=open]:slide-in-from-top inset-x-0 top-0 h-auto border-b pt-safe',
        side === 'bottom'
          && 'data-[state=closed]:slide-out-to-bottom data-[state=open]:slide-in-from-bottom inset-x-0 bottom-0 h-auto border-t pb-safe',
        props.class)"
      v-bind="{ ...$attrs, ...forwarded }"
    >
      <slot />

      <DialogClose
        v-if="!props.hideClose"
        class="ring-offset-background focus:ring-ring data-[state=open]:bg-secondary absolute top-[calc(1rem+var(--safe-top))] right-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none"
      >
        <X class="size-4" />
        <span class="sr-only">{{ t('actions.close') }}</span>
      </DialogClose>
    </DialogContent>
  </DialogPortal>
</template>
