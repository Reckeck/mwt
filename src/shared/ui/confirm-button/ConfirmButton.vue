<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'

import { Button } from '@/shared/ui/button'

import type { ConfirmButtonProps } from './confirm-button.types'

const props = withDefaults(defineProps<ConfirmButtonProps>(), { disabled: false, timeout: 3000 })
const emit = defineEmits<{ confirm: [] }>()

/**
 * Two-step destructive action without a modal: the first click arms the
 * button, the second (within `timeout`) confirms, anything else disarms it.
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
    v-if="armed"
    variant="destructive"
    size="sm"
    class="h-8 shrink-0 px-2.5"
    :disabled="props.disabled"
    @click="click"
    @blur="disarm"
  >
    {{ props.confirmLabel }}
  </Button>
  <Button
    v-else
    variant="ghost"
    size="icon"
    class="size-8 shrink-0 text-muted-foreground hover:text-destructive"
    :title="props.label"
    :aria-label="props.label"
    :disabled="props.disabled"
    @click="click"
  >
    <slot />
  </Button>
</template>
