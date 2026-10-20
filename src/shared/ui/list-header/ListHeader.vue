<script setup lang="ts">
import { Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import type { ListHeaderProps } from './list-header.types'

/**
 * The row above a list of plans (limits, goals, reminders…): what the list is
 * for, its main action and room for a secondary one (`#actions`).
 */
const props = withDefaults(defineProps<ListHeaderProps>(), { actionLabel: undefined, hideAction: false })
const emit = defineEmits<{ action: [] }>()
defineSlots<{ actions?(): unknown }>()
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-2">
    <!-- basis-64: on a narrow screen the buttons wrap below the text instead of squeezing it under them. -->
    <p class="min-w-0 flex-1 basis-64 text-sm text-muted-foreground">{{ props.intro }}</p>
    <div v-if="$slots.actions || (props.actionLabel && !props.hideAction)" class="flex flex-wrap items-center gap-2">
      <slot name="actions" />
      <Button v-if="props.actionLabel && !props.hideAction" @click="emit('action')">
        <Plus class="size-4" />
        {{ props.actionLabel }}
      </Button>
    </div>
  </div>
</template>
