<script setup lang="ts">
import type { PopoverContentEmits, PopoverContentProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'
import { reactiveOmit } from '@vueuse/core'
import {
  PopoverContent,
  PopoverPortal,
  injectPopoverRootContext,
  useForwardPropsEmits,
} from 'reka-ui'
import { useLatePortal } from '@/shared/lib/late-portal'
import { cn } from '@/shared/lib/utils'
import { keepOpenOnToast } from '@/shared/lib/toast-outside'

defineOptions({
  inheritAttrs: false,
})

const props = withDefaults(
  defineProps<PopoverContentProps & { class?: HTMLAttributes['class']; overlay?: string }>(),
  {
    align: 'center',
    sideOffset: 4,
  },
)
const emits = defineEmits<PopoverContentEmits>()

const delegatedProps = reactiveOmit(props, 'class', 'overlay')

const forwarded = useForwardPropsEmits(delegatedProps, emits)
const portalled = useLatePortal(injectPopoverRootContext().open)
</script>

<template>
  <PopoverPortal v-if="portalled">
    <PopoverContent
      data-slot="popover-content"
      :data-overlay="props.overlay"
      @pointer-down-outside="keepOpenOnToast"
      @interact-outside="keepOpenOnToast"
      v-bind="{ ...$attrs, ...forwarded }"
      :class="
        cn(
          'bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-72 max-h-(--reka-popover-content-available-height) max-w-(--reka-popover-content-available-width) overflow-y-auto rounded-md border p-4 shadow-md origin-(--reka-popover-content-transform-origin) outline-hidden',
          props.class,
        )
      "
    >
      <slot />
    </PopoverContent>
  </PopoverPortal>
</template>
