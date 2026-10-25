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

import { useCoarsePointer, useIsPhone } from '@/shared/lib/phone'
import { Drawer, DrawerClose, DrawerContent, DrawerDescription, DrawerTitle } from '@/shared/ui/drawer'

import { dialogWidths, type DialogProps } from './dialog.constants'
import { keepOpenOnToast } from '@/shared/lib/toast-outside'

const props = withDefaults(
  defineProps<DialogProps>(),
  { size: 'md' },
)

const emit = defineEmits<{ 'update:open': [boolean] }>()

const widths = dialogWidths

const { t } = useI18n()
/** On a phone the same dialog is a sheet from the bottom that a swipe closes. */
const phone = useIsPhone()
const coarse = useCoarsePointer()

const FIELD = '[autofocus], input:not([type="hidden"]):not([disabled]), textarea:not([disabled]), select:not([disabled]), [role="combobox"]'

/**
 * Opening lands on the field to fill — the one marked `autofocus`, else the
 * first — not on the close button Reka would pick. A dialog without fields
 * keeps Reka's default.
 */
function focusFirstField(event: Event) {
  const panel = event.currentTarget instanceof HTMLElement ? event.currentTarget : null
  // A touch screen: a focused field opens the keyboard over half the sheet before the
  // user chose to type. Only a field marked `autofocus` (a password: typing is all there
  // is to do) takes it; otherwise the panel does (not the «×» Reka would pick).
  const marked = panel?.querySelector<HTMLElement>('[data-dialog-body] [autofocus]')
  if (coarse.value) {
    event.preventDefault()
    ;(marked ?? panel)?.focus()
    return
  }
  // A field that is not drawn (a `hidden` file input behind a button) cannot take focus: skip it.
  const drawn = (node: HTMLElement) => node.getClientRects().length > 0
  const field = marked ?? [...(panel?.querySelectorAll<HTMLElement>(`[data-dialog-body] :is(${FIELD})`) ?? [])].find(drawn)
  if (!field) return
  event.preventDefault()
  field.focus()
}
</script>

<template>
  <Drawer v-if="phone" :open="props.open" @update:open="(value) => emit('update:open', value)">
    <DrawerContent
      v-bind="props.description ? {} : { 'aria-describedby': undefined }"
      tabindex="-1"
      :layer="props.overlay"
      @open-auto-focus="focusFirstField"
    >
      <header class="flex items-start gap-3 border-b border-border px-4 pt-2 pb-3">
        <div class="min-w-0 flex-1">
          <DrawerTitle class="truncate">{{ props.title }}</DrawerTitle>
          <DrawerDescription v-if="props.description" class="mt-0.5">{{ props.description }}</DrawerDescription>
        </div>
        <DrawerClose
          class="-mr-1 flex size-10 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <X class="size-4" />
          <span class="sr-only">{{ t('actions.close') }}</span>
        </DrawerClose>
      </header>

      <div data-dialog-body class="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4">
        <slot />
      </div>

      <footer v-if="$slots.footer" class="flex flex-wrap items-center justify-end gap-2 border-t border-border p-4">
        <slot name="footer" />
      </footer>
    </DrawerContent>
  </Drawer>

  <DialogRoot v-else :open="props.open" @update:open="(value) => emit('update:open', value)">
    <DialogPortal>
      <DialogOverlay class="fixed inset-0 z-50 bg-black/60 data-[state=open]:animate-overlay-in data-[state=closed]:animate-overlay-out" />
      <DialogContent
        class="fixed left-1/2 top-1/2 z-50 flex max-h-[calc(90dvh-var(--safe-top)-var(--safe-bottom))] w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card text-card-foreground shadow-lg focus:outline-none data-[state=open]:animate-panel-in data-[state=closed]:animate-panel-out"
        :class="widths[props.size]"
        :data-overlay="props.overlay"
        v-bind="props.description ? {} : { 'aria-describedby': undefined }"
        @open-auto-focus="focusFirstField"
        @pointer-down-outside="keepOpenOnToast"
        @interact-outside="keepOpenOnToast"
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

        <div data-dialog-body class="min-h-0 flex-1 overflow-y-auto p-4">
          <slot />
        </div>

        <footer v-if="$slots.footer" class="flex items-center justify-end gap-2 border-t border-border p-4">
          <slot name="footer" />
        </footer>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

