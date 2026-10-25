<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerTitle, DrawerTrigger } from '@/shared/ui/drawer'
import { Kbd } from '@/shared/ui/kbd'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import { GLANCE_TRIGGER_CLASS } from './glance.constants'
import type { GlanceProps } from './glance.types'

/**
 * A quick glance (Paths 1 · 2–4): a figure or a name opens what it is made of
 * without leaving the page — a card by the figure on a computer, a bottom sheet
 * on a phone. The page under it does not move. Enter goes to `openTo`, Esc and a
 * click aside close it; on a phone «Назад» closes the sheet.
 */
const props = defineProps<GlanceProps>()
const open = defineModel<boolean>('open', { default: false })
defineSlots<{
  /** The figure or the name itself. */
  default(): unknown
  /** The card's body. */
  body(): unknown
  /** The next steps besides «Открыть →» («Задать лимит»). */
  actions?(): unknown
}>()

const { t } = useI18n()
const router = useRouter()
const phone = useIsPhone()

function go() {
  if (!props.openTo) return
  open.value = false
  void router.push(props.openTo)
}
function onKey(event: KeyboardEvent) {
  // Enter on a button inside presses that button, not the link.
  if (event.key !== 'Enter' || (event.target as HTMLElement).closest('button, a, input')) return
  event.preventDefault()
  go()
}
</script>

<template>
  <Popover v-if="!phone" v-model:open="open">
    <PopoverTrigger :class="cn(GLANCE_TRIGGER_CLASS, props.triggerClass)" :data-glance-trigger="props.kind ?? ''" :data-entity="props.entity">
      <slot />
    </PopoverTrigger>
    <PopoverContent
      align="start"
      :side-offset="8"
      class="flex w-[360px] max-w-[calc(100vw-2rem)] flex-col gap-0 rounded-[10px] p-0"
      data-glance
      @keydown="onKey"
    >
      <div class="flex flex-col gap-2.5 p-3.5 text-sm">
        <slot name="body" />
        <p v-if="props.openTo" class="flex gap-3 text-xs text-muted-foreground">
          <span><Kbd>Enter</Kbd> {{ t('glance.open') }}</span>
          <span><Kbd>Esc</Kbd> {{ t('glance.close') }}</span>
        </p>
      </div>
      <div v-if="props.openTo || $slots.actions" class="flex items-center gap-2 border-t border-border px-3.5 py-2.5">
        <slot name="actions" />
        <span class="flex-1" />
        <Button v-if="props.openTo" variant="link" size="sm" class="h-auto px-0 text-sm" data-glance-open @click="go">
          {{ props.openLabel }} →
        </Button>
      </div>
    </PopoverContent>
  </Popover>
  <Drawer v-else v-model:open="open">
    <DrawerTrigger :class="cn(GLANCE_TRIGGER_CLASS, props.triggerClass)" :data-glance-trigger="props.kind ?? ''" :data-entity="props.entity">
      <slot />
    </DrawerTrigger>
    <DrawerContent class="gap-3 px-4 pt-2 pb-5" data-glance>
      <DrawerTitle class="sr-only">{{ props.title }}</DrawerTitle>
      <div class="flex flex-col gap-2.5 text-sm"><slot name="body" /></div>
      <div v-if="props.openTo || $slots.actions" class="grid auto-cols-fr grid-flow-col gap-2 [&_button]:h-11">
        <slot name="actions" />
        <Button v-if="props.openTo" data-glance-open @click="go">{{ props.openLabel }}</Button>
      </div>
    </DrawerContent>
  </Drawer>
</template>
