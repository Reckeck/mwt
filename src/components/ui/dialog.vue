<script setup lang="ts">
import {
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from 'reka-ui'

const props = withDefaults(
  defineProps<{
    /** Controlled open state; omit for an uncontrolled dialog. */
    open?: boolean
    title: string
    description?: string
    /** Tailwind max-width class for the panel. */
    size?: 'sm' | 'md' | 'lg' | 'xl'
  }>(),
  { size: 'md' },
)

const emit = defineEmits<{ 'update:open': [boolean] }>()

const widths = {
  sm: 'max-w-sm',
  md: 'max-w-lg',
  lg: 'max-w-2xl',
  xl: 'max-w-4xl',
} as const
</script>

<template>
  <DialogRoot :open="props.open" @update:open="(v) => emit('update:open', v)">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 data-[state=open]:animate-overlay-in" />
      <DialogContent
        class="fixed left-1/2 top-1/2 z-50 flex max-h-[90dvh] w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-lg focus:outline-none data-[state=open]:animate-panel-in"
        :class="widths[props.size]"
      >
        <header class="flex items-start gap-3 border-b border-border p-4">
          <div class="min-w-0 flex-1">
            <DialogTitle class="truncate font-semibold">{{ props.title }}</DialogTitle>
            <DialogDescription v-if="props.description" class="mt-0.5 text-sm text-muted-foreground">
              {{ props.description }}
            </DialogDescription>
          </div>
          <slot name="close" />
        </header>

        <div class="min-h-0 flex-1 overflow-y-auto p-4">
          <slot />
        </div>

        <footer v-if="$slots.footer" class="flex items-center gap-2 border-t border-border p-4">
          <slot name="footer" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

