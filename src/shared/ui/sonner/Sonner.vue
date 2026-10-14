<script lang="ts" setup>
import { CircleCheck, CircleX, Info, LoaderCircle, TriangleAlert, X } from 'lucide-vue-next'
import type { ToasterProps } from 'vue-sonner'
import { Toaster } from 'vue-sonner'
// vue-sonner 2 ships its styles apart; without them the toasts render below the
// window, where `body { overflow: hidden }` hides every one of them.
import 'vue-sonner/style.css'
import { cn } from '@/shared/lib/utils'

const props = defineProps<ToasterProps>()
</script>

<template>
  <Toaster
    :class="cn('toaster group', props.class)"
    :style="{
      '--normal-bg': 'var(--popover)',
      '--normal-text': 'var(--popover-foreground)',
      '--normal-border': 'var(--border)',
      '--border-radius': 'var(--radius)',
    }"
    v-bind="props"
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
