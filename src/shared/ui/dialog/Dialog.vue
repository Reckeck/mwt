<script setup lang="ts">
import { X } from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import {
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogOverlay,
  DialogPortal,
  DialogRoot,
  DialogTitle,
} from 'reka-ui'

import { dialogWidths, type DialogProps } from './dialog.constants'

const props = withDefaults(
  defineProps<DialogProps>(),
  { size: 'md' },
)

const emit = defineEmits<{ 'update:open': [boolean] }>()

const widths = dialogWidths

const { t } = useI18n()
</script>

<template>
  <DialogRoot :open="props.open" @update:open="(value) => emit('update:open', value)">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 data-[state=open]:animate-overlay-in" />
      <DialogContent
        class="fixed left-1/2 top-1/2 z-50 flex max-h-[90dvh] w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-lg focus:outline-none data-[state=open]:animate-panel-in"
        :class="widths[props.size]"
        v-bind="props.description ? {} : { 'aria-describedby': undefined }"
      >
        <header class="flex items-start gap-3 border-b border-border p-4">
          <div class="min-w-0 flex-1">
            <DialogTitle class="truncate font-semibold">{{ props.title }}</DialogTitle>
            <DialogDescription v-if="props.description" class="mt-0.5 text-sm text-muted-foreground">
              {{ props.description }}
            </DialogDescription>
          </div>
          <DialogClose
            class="-mr-1 -mt-1 flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X class="size-4" />
            <span class="sr-only">{{ t('actions.close') }}</span>
          </DialogClose>
        </header>

        <div class="min-h-0 flex-1 overflow-y-auto p-4">
          <slot />
        </div>

        <footer v-if="$slots.footer" class="flex items-center justify-end gap-2 border-t border-border p-4">
          <slot name="footer" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

