<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Camera, ChevronDown, FileUp, Link, Plus } from 'lucide-vue-next'
import { DropdownMenuContent, DropdownMenuItem, DropdownMenuPortal, DropdownMenuRoot, DropdownMenuTrigger } from 'reka-ui'

import { Button } from '@/shared/ui/button'
import { MENU_CONTENT_CLASS, MENU_ITEM_CLASS, MENU_ITEM_ICON_CLASS } from '@/shared/ui/context-menu'
import { Drawer, DrawerContent, DrawerDescription, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverAnchor, PopoverContent } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'

import type { LinkDraft } from '../../attachment.types'
import AttachmentLinkForm from './AttachmentLinkForm.vue'

/**
 * `Add ▾` (Attach 2, 4). On a computer a menu — File…, Link — and the
 * link form as a popover under the button; on a phone a sheet with the camera
 * first, since a receipt is usually photographed on the spot.
 */
const props = defineProps<{
  pending?: boolean
  linkUrl?: string
  /** `File or link`: a dashed button of one line in a form (Attach 5), instead of `Add ▾`. */
  label?: string
}>()
const linkOpen = defineModel<boolean>('linkOpen', { default: false })
const emit = defineEmits<{ files: [files: File[]]; link: [draft: LinkDraft] }>()

const { t } = useI18n()
const phone = useIsPhone()
const sheetOpen = ref(false)
const fileInput = ref<HTMLInputElement | null>(null)
const cameraInput = ref<HTMLInputElement | null>(null)

function picked(event: Event) {
  const input = event.target as HTMLInputElement
  const files = Array.from(input.files ?? [])
  input.value = ''
  if (files.length) emit('files', files)
}

function choose(which: 'camera' | 'file' | 'link') {
  sheetOpen.value = false
  // after the menu has closed and given focus back, or the popover takes it for a click outside
  if (which === 'link') setTimeout(() => (linkOpen.value = true))
  else (which === 'camera' ? cameraInput : fileInput).value?.click()
}

const PHONE_ROWS = [
  { id: 'camera', icon: Camera, title: 'attachments.camera', hint: 'attachments.cameraHint' },
  { id: 'file', icon: FileUp, title: 'attachments.chooseFile', hint: 'attachments.chooseFileHint' },
  { id: 'link', icon: Link, title: 'attachments.link', hint: 'attachments.linkHint' },
] as const
</script>

<template>
  <input ref="fileInput" type="file" multiple class="hidden" data-attachment-input @change="picked" />
  <input ref="cameraInput" type="file" accept="image/*" capture="environment" class="hidden" @change="picked" />

  <template v-if="phone">
    <Button
      variant="outline"
      size="sm"
      :class="['h-10', props.label && 'border-dashed']"
      data-attachment-add
      @click="sheetOpen = true"
    >
      <Plus class="size-4" />{{ props.label ?? t('actions.add') }}<ChevronDown v-if="!props.label" class="size-3.5" />
    </Button>
    <Drawer v-model:open="sheetOpen">
      <DrawerContent layer="attachments" data-attachment-sheet>
        <DrawerHeader class="px-4 pb-2 text-left">
          <DrawerTitle class="text-base">{{ t('attachments.addTo') }}</DrawerTitle>
          <DrawerDescription class="sr-only">{{ t('attachments.addTo') }}</DrawerDescription>
        </DrawerHeader>
        <div class="mx-4 divide-y rounded-xl border px-3">
          <Button
            v-for="row in PHONE_ROWS"
            :key="row.id"
            variant="ghost"
            size="block"
            class="min-h-14 w-full gap-3 rounded-none px-0 py-2 hover:bg-transparent"
            :data-attachment-pick="row.id"
            @click="choose(row.id)"
          >
            <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-secondary">
              <component :is="row.icon" class="size-4" />
            </span>
            <span class="grid">
              <span class="text-sm font-medium">{{ t(row.title) }}</span>
              <span class="text-xs text-muted-foreground">{{ t(row.hint) }}</span>
            </span>
          </Button>
        </div>
        <div class="mt-3 border-t px-4 pt-3 pb-[calc(1.5rem+var(--safe-bottom))]">
          <Button variant="outline" class="h-12 w-full" @click="sheetOpen = false">{{ t('actions.cancel') }}</Button>
        </div>
      </DrawerContent>
    </Drawer>
    <Drawer v-model:open="linkOpen">
      <DrawerContent layer="attachment-actions">
        <DrawerHeader class="sr-only">
          <DrawerTitle>{{ t('attachments.link') }}</DrawerTitle>
          <DrawerDescription>{{ t('attachments.linkHint') }}</DrawerDescription>
        </DrawerHeader>
        <div class="px-4 pt-2 pb-[calc(1.5rem+var(--safe-bottom))]">
          <AttachmentLinkForm :url="props.linkUrl" :pending="props.pending" @submit="emit('link', $event)" @cancel="linkOpen = false" />
        </div>
      </DrawerContent>
    </Drawer>
  </template>

  <Popover v-else v-model:open="linkOpen">
    <PopoverAnchor as-child>
      <div>
        <DropdownMenuRoot :modal="false">
          <DropdownMenuTrigger as-child>
            <Button
              variant="outline"
              size="sm"
              :class="[props.label && 'h-7 border-dashed', linkOpen && 'border-primary']"
              data-attachment-add
            >
              <Plus class="size-4" />{{ props.label ?? t('actions.add') }}<ChevronDown v-if="!props.label" class="size-3.5" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuPortal>
            <DropdownMenuContent
              :class="[MENU_CONTENT_CLASS, 'w-[min(17.5rem,calc(100vw-2rem))]']"
              :align="props.label ? 'start' : 'end'"
              :side-offset="4"
              @close-auto-focus.prevent
            >
              <DropdownMenuItem :class="[MENU_ITEM_CLASS, 'items-start']" data-attachment-pick="file" @select="choose('file')">
                <FileUp :class="[MENU_ITEM_ICON_CLASS, 'mt-0.5']" />
                <span class="grid">
                  <span class="font-medium">{{ t('attachments.file') }}</span>
                  <span class="text-xs text-muted-foreground">{{ t('attachments.fileHint') }}</span>
                </span>
              </DropdownMenuItem>
              <DropdownMenuItem :class="[MENU_ITEM_CLASS, 'items-start']" data-attachment-pick="link" @select="choose('link')">
                <Link :class="[MENU_ITEM_ICON_CLASS, 'mt-0.5']" />
                <span class="grid">
                  <span class="font-medium">{{ t('attachments.link') }}</span>
                  <span class="text-xs text-muted-foreground">{{ t('attachments.linkHint') }}</span>
                </span>
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenuPortal>
        </DropdownMenuRoot>
      </div>
    </PopoverAnchor>
    <PopoverContent overlay="attachments" :align="props.label ? 'start' : 'end'" class="w-[min(21.25rem,calc(100vw-2rem))] p-3">
      <AttachmentLinkForm :url="props.linkUrl" :pending="props.pending" @submit="emit('link', $event)" @cancel="linkOpen = false" />
    </PopoverContent>
  </Popover>
</template>
