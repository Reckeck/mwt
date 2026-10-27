<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerDescription, DrawerFooter, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'

import type { ConfirmButtonProps } from './confirm-button.types'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<ConfirmButtonProps>(), { disabled: false, showLabel: false })
const emit = defineEmits<{ confirm: [] }>()

/**
 * Two-step destructive action. The button never changes: the first click opens a
 * confirmation anchored to it (a small popover on a desktop, a sheet from below on a
 * phone) and the red button in that layer confirms. Esc, a click outside and «Cancel»
 * close it without acting. Nothing is laid over the neighbours (rule 17).
 */
const { t } = useI18n()
const phone = useIsPhone()
const open = defineModel<boolean>('open', { default: false })
const confirmRef = ref<InstanceType<typeof Button> | null>(null)

function confirm() {
  open.value = false
  emit('confirm')
}

/** The red button, not «Cancel», takes the focus: Enter confirms what was asked for. */
function focusConfirm(event: Event) {
  event.preventDefault()
  void nextTick(() => (confirmRef.value?.$el as HTMLElement | undefined)?.focus())
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button
        :variant="props.showLabel ? 'outline' : 'ghost'"
        :size="props.showLabel ? 'default' : 'icon-sm'"
        v-bind="$attrs"
        data-slot="confirm-button"
        :data-armed="open || undefined"
        :class="cn('shrink-0 hover:text-destructive', !props.showLabel && 'text-muted-foreground', open && 'text-destructive')"
        :title="props.label"
        :aria-label="props.label"
        :disabled="props.disabled"
      >
        <slot />
        <span v-if="props.showLabel">{{ props.label }}</span>
      </Button>
    </PopoverTrigger>

    <PopoverContent v-if="!phone" overlay="confirm" side="bottom" align="start" :collision-padding="8" class="grid w-72 gap-3 p-3" @open-auto-focus="focusConfirm">
      <div class="grid gap-1">
        <p class="text-sm font-medium">{{ props.confirmLabel }}</p>
        <p v-if="props.description" class="text-xs text-muted-foreground">{{ props.description }}</p>
      </div>
      <div class="flex justify-end gap-2">
        <Button variant="outline" size="sm" @click="open = false">{{ t('actions.cancel') }}</Button>
        <Button ref="confirmRef" variant="destructive" size="sm" data-confirm-action @click="confirm">{{ props.actionLabel ?? t('actions.delete') }}</Button>
      </div>
    </PopoverContent>
  </Popover>

  <Drawer v-if="phone" v-model:open="open">
    <DrawerContent layer="confirm">
      <DrawerHeader class="px-4 pb-2 text-left">
        <DrawerTitle class="text-base">{{ props.confirmLabel }}</DrawerTitle>
        <DrawerDescription :class="cn('text-sm', !props.description && 'sr-only')">{{ props.description || props.label }}</DrawerDescription>
      </DrawerHeader>
      <DrawerFooter>
        <Button variant="destructive" class="h-12 w-full" data-confirm-action @click="confirm">{{ props.actionLabel ?? t('actions.delete') }}</Button>
        <Button variant="outline" class="h-12 w-full" @click="open = false">{{ t('actions.cancel') }}</Button>
      </DrawerFooter>
    </DrawerContent>
  </Drawer>
</template>
