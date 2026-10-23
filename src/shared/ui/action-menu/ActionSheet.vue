<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { visibleActions } from '@/shared/ui/context-menu'
import { Drawer, DrawerContent, DrawerDescription, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'
import { cn } from '@/shared/lib/utils'

import type { ActionSheetProps } from './action-menu.types'

/**
 * The phone twin of the right-click menu (Paths 2 · 11): a sheet from below
 * with what the item is on top and the same actions in the same order, rows
 * a finger can hit. A long press on a row or its «⋯» opens it.
 */
const props = defineProps<ActionSheetProps>()
const open = defineModel<boolean>('open', { default: false })

const { t } = useI18n()

function pick(action: ActionSheetProps['actions'][number]) {
  open.value = false
  action.run?.()
}

const ROW = 'h-auto min-h-12 w-full justify-start gap-3 rounded-none px-1 text-left text-sm font-normal'
</script>

<template>
  <Drawer v-model:open="open">
    <DrawerContent data-action-sheet>
      <DrawerHeader class="flex-row items-start justify-between gap-3 px-4 pb-2 text-left">
        <div class="grid min-w-0 gap-0.5">
          <DrawerTitle class="truncate text-base">{{ props.title || t('actions.more') }}</DrawerTitle>
          <DrawerDescription :class="cn('truncate text-xs', !props.description && 'sr-only')">
            {{ props.description || t('actions.more') }}
          </DrawerDescription>
        </div>
        <b v-if="props.value" class="shrink-0 text-base tabular-nums">{{ props.value }}</b>
      </DrawerHeader>
      <div class="grid px-4 pb-[calc(1.5rem+var(--safe-bottom))]">
        <template v-for="(action, index) in visibleActions(props.actions)" :key="action.id">
          <div v-if="action.separated && index > 0" class="my-1 h-px bg-border" />
          <Button
            variant="ghost"
            :class="cn(ROW, action.destructive && 'text-destructive hover:text-destructive')"
            :disabled="Boolean(action.disabled)"
            @click="pick(action)"
          >
            <component :is="action.icon" v-if="action.icon" class="size-5 opacity-70" />
            <span class="flex-1">{{ action.label }}</span>
          </Button>
        </template>
      </div>
    </DrawerContent>
  </Drawer>
</template>
