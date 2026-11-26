<script lang="ts" setup>
import { CircleCheck, CircleX, Info, LoaderCircle, TriangleAlert, X } from 'lucide-vue-next'
import { computed } from 'vue'
import type { ToasterProps } from 'vue-sonner'
import { Toaster } from 'vue-sonner'
// vue-sonner 2 ships its styles apart; without them the toasts render below the
// window, where `body { overflow: hidden }` hides every one of them.
import 'vue-sonner/style.css'
import { cn } from '@/shared/lib/utils'

const props = defineProps<ToasterProps>()

/** The Undo action reads as a link on the inverse toast; a caller's own options win. */
const toastOptions = computed<ToasterProps['toastOptions']>(() => ({
  ...props.toastOptions,
  classes: { actionButton: '!bg-transparent !font-semibold !text-inverse-foreground underline', ...props.toastOptions?.classes },
}))
</script>

<template>
  <Toaster
    :class="cn('toaster group', props.class)"
    :style="{
      '--normal-bg': 'var(--surface-inverse)',
      '--normal-text': 'var(--inverse-foreground)',
      '--normal-border': 'var(--surface-inverse)',
      '--border-radius': 'var(--radius-md)',
      '--success-bg': 'var(--surface-inverse)',
      '--success-text': 'var(--inverse-foreground)',
      '--success-border': 'var(--surface-inverse)',
      '--error-bg': 'var(--surface-inverse)',
      '--error-text': 'var(--inverse-foreground)',
      '--error-border': 'var(--surface-inverse)',
    }"
    v-bind="props"
    :toast-options="toastOptions"
  >
    <template #success-icon>
      <CircleCheck class="size-4" />
    </template>
    <template #info-icon>
      <Info class="size-4" />
    </template>
    <template #warning-icon>
      <TriangleAlert class="size-4" />
    </template>
    <template #error-icon>
      <CircleX class="size-4" />
    </template>
    <template #loading-icon>
      <LoaderCircle class="size-4 animate-spin" />
    </template>
    <template #close-icon>
      <X class="size-4" />
    </template>
  </Toaster>
</template>
