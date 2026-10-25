<script lang="ts" setup>
import type { DrawerContentEmits, DrawerContentProps } from 'reka-ui'
import type { HTMLAttributes } from 'vue'
import { reactiveOmit } from '@vueuse/core'
import { DrawerContent, DrawerHandle, DrawerPortal, useForwardPropsEmits } from 'reka-ui'

import { cn } from '@/shared/lib/utils'

import DrawerOverlay from './DrawerOverlay.vue'
import { keepOpenOnToast } from '@/shared/lib/toast-outside'

defineOptions({ inheritAttrs: false })

/**
 * A panel that slides in from an edge and follows the finger. From the bottom
 * (the default) it stops below the status bar and keeps clear of the gesture bar.
 * `overlay: false` leaves the page usable behind it (a non-modal drawer).
 * `layer` is the name for `data-overlay` (`overlay` is taken by the backdrop).
 */
const props = withDefaults(defineProps<DrawerContentProps & { class?: HTMLAttributes['class']; overlay?: boolean; layer?: string }>(), {
  overlay: true,
})
const emits = defineEmits<DrawerContentEmits>()
const delegatedProps = reactiveOmit(props, 'class', 'overlay', 'layer')
const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <DrawerPortal>
    <DrawerOverlay v-if="props.overlay" />
    <DrawerContent
      data-slot="drawer-content"
      :data-overlay="props.layer"
      @pointer-down-outside="keepOpenOnToast"
      @interact-outside="keepOpenOnToast"
      v-bind="{ ...$attrs, ...forwarded }"
      :class="
        cn(
          'group/drawer-content fixed z-50 flex h-auto flex-col border-border bg-card text-card-foreground focus:outline-none',
          'will-change-transform transform-[translate3d(var(--drawer-swipe-movement-x,0px),calc(var(--drawer-swipe-movement-y,0px)_+_var(--drawer-snap-point-offset,0px)),0)]',
          'transition-transform duration-500 ease-[cubic-bezier(0.32,0.72,0,1)] data-swiping:duration-0 data-swiping:select-none',
          'data-[state=closed]:animate-out data-[state=open]:animate-in',
          'data-[swipe-direction=down]:inset-x-0 data-[swipe-direction=down]:bottom-0 data-[swipe-direction=down]:max-h-[calc(100dvh-var(--safe-top)-1.5rem)] data-[swipe-direction=down]:rounded-t-xl data-[swipe-direction=down]:border-t data-[swipe-direction=down]:pb-safe data-[swipe-direction=down]:data-[state=closed]:slide-out-to-bottom data-[swipe-direction=down]:data-[state=open]:slide-in-from-bottom',
          'data-[swipe-direction=up]:inset-x-0 data-[swipe-direction=up]:top-0 data-[swipe-direction=up]:max-h-[80dvh] data-[swipe-direction=up]:rounded-b-xl data-[swipe-direction=up]:border-b data-[swipe-direction=up]:pt-safe data-[swipe-direction=up]:data-[state=closed]:slide-out-to-top data-[swipe-direction=up]:data-[state=open]:slide-in-from-top',
          'data-[swipe-direction=right]:inset-y-0 data-[swipe-direction=right]:right-0 data-[swipe-direction=right]:w-3/4 data-[swipe-direction=right]:border-l data-[swipe-direction=right]:pt-safe data-[swipe-direction=right]:pb-safe data-[swipe-direction=right]:sm:max-w-md data-[swipe-direction=right]:data-[state=closed]:slide-out-to-right data-[swipe-direction=right]:data-[state=open]:slide-in-from-right',
          'data-[swipe-direction=left]:inset-y-0 data-[swipe-direction=left]:left-0 data-[swipe-direction=left]:w-3/4 data-[swipe-direction=left]:border-r data-[swipe-direction=left]:pt-safe data-[swipe-direction=left]:pb-safe data-[swipe-direction=left]:sm:max-w-md data-[swipe-direction=left]:data-[state=closed]:slide-out-to-left data-[swipe-direction=left]:data-[state=open]:slide-in-from-left',
          props.class,
        )
      "
    >
      <DrawerHandle
        class="mx-auto mt-2 hidden h-1.5 w-12 shrink-0 rounded-full bg-muted-foreground/30 group-data-[swipe-direction=down]/drawer-content:block"
      />
      <slot />
    </DrawerContent>
  </DrawerPortal>
</template>
