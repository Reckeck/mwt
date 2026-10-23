<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'

import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

import type { ConfirmButtonProps } from './confirm-button.types'

const props = withDefaults(defineProps<ConfirmButtonProps>(), { disabled: false, timeout: 3000, showLabel: false })
const emit = defineEmits<{ confirm: [] }>()

/**
 * Two-step destructive action without a modal: the first click arms the
 * button, the second (within `timeout`) confirms, anything else disarms it.
 *
 * One element in both states (rule 17): the icon button keeps its size and its
 * focus; the armed «Delete?» is a label laid over its neighbours from the
 * button's right edge, so nothing in the row moves.
 */
const armed = ref(false)
let timer: ReturnType<typeof setTimeout> | undefined

function disarm() {
  armed.value = false
  clearTimeout(timer)
}

function click() {
  if (!armed.value) {
    armed.value = true
    timer = setTimeout(disarm, props.timeout)
    return
  }
  disarm()
  emit('confirm')
}

onBeforeUnmount(() => clearTimeout(timer))
</script>

<template>
  <Button
    :variant="props.showLabel ? 'outline' : 'ghost'"
    :size="props.showLabel ? 'default' : 'icon-sm'"
    data-slot="confirm-button"
    :data-armed="armed || undefined"
    :class="
      cn(
        'relative shrink-0 hover:text-destructive',
        !props.showLabel && 'text-muted-foreground',
        armed && 'z-10 text-destructive-foreground hover:text-destructive-foreground',
      )
    "
    :title="armed ? props.confirmLabel : props.label"
    :aria-label="armed ? props.confirmLabel : props.label"
    :disabled="props.disabled"
    @click="click"
    @blur="disarm"
    @keydown.escape="disarm"
  >
    <slot />
    <span v-if="props.showLabel">{{ props.label }}</span>
    <span
      v-if="armed"
      aria-hidden="true"
      class="pointer-events-none absolute inset-y-0 right-0 flex items-center whitespace-nowrap rounded-md bg-destructive px-2.5 text-sm font-medium text-destructive-foreground shadow-sm"
    >
      {{ props.confirmLabel }}
    </span>
  </Button>
</template>
