<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { Spinner } from '@/shared/ui/spinner'
import { cn } from '@/shared/lib/utils'

import { buttonVariants, type ButtonProps } from './button.variants'

defineOptions({ inheritAttrs: false })

const props = withDefaults(defineProps<ButtonProps>(), { variant: 'default', size: 'default', loading: false })

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
const disabled = computed(() => props.loading || (rest.value.disabled !== undefined && rest.value.disabled !== false))
</script>

<template>
  <!-- `type` defaults to button; callers pass `type="submit"` through $attrs to override. -->
  <button
    type="button"
    :class="cn(buttonVariants({ variant: props.variant, size: props.size }), props.loading && 'relative', cls)"
    v-bind="rest"
    :disabled="disabled"
    :aria-busy="props.loading || undefined"
  >
    <!-- `contents`: the label's parts stay flex items of the button (gap, alignment). -->
    <span :class="cn('contents', props.loading && 'invisible')"><slot /></span>
    <span v-if="props.loading" class="absolute inset-0 flex items-center justify-center"><Spinner /></span>
  </button>
</template>