<script setup lang="ts">
import type { VariantProps } from "class-variance-authority"
import type { ToggleGroupRootEmits, ToggleGroupRootProps } from "reka-ui"
import type { HTMLAttributes } from "vue"
import type { toggleVariants } from '@/shared/ui/toggle'
import { reactiveOmit } from "@vueuse/core"
import { ToggleGroupRoot, useForwardPropsEmits } from "reka-ui"
import { provide } from "vue"
import { cn } from '@/shared/lib/utils'

/** `range` is the range switch (1M/3M/1Y/All): `segmented` in a pill track. */
type ToggleGroupVariants = Omit<VariantProps<typeof toggleVariants>, 'variant'> & {
  variant?: NonNullable<VariantProps<typeof toggleVariants>['variant']> | 'range'
}

const props = withDefaults(defineProps<ToggleGroupRootProps & {
  class?: HTMLAttributes["class"]
  variant?: ToggleGroupVariants["variant"]
  size?: ToggleGroupVariants["size"]
  spacing?: number
}>(), {
  spacing: 0,
})

const emits = defineEmits<ToggleGroupRootEmits>()

provide("toggleGroup", {
  variant: props.variant,
  size: props.size,
  spacing: props.spacing,
})

const delegatedProps = reactiveOmit(props, "class", "size", "variant")
const forwarded = useForwardPropsEmits(delegatedProps, emits)
</script>

<template>
  <ToggleGroupRoot
    v-slot="slotProps"
    data-slot="toggle-group"
    :data-size="size"
    :data-variant="variant"
    :data-spacing="spacing"
    :style="{
      '--gap': spacing,
    }"
    v-bind="forwarded"
    :class="
      cn(
        'group/toggle-group flex w-fit items-center gap-[--spacing(var(--gap))] rounded-md',
        'data-[variant=segmented]:rounded-lg data-[variant=segmented]:bg-muted data-[variant=segmented]:p-1',
        'data-[variant=range]:rounded-full data-[variant=range]:bg-muted data-[variant=range]:p-[3px]',
        props.class,
      )
    "
  >
    <slot v-bind="slotProps" />
  </ToggleGroupRoot>
</template>
