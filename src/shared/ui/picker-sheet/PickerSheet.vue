<script setup lang="ts">
import { AdaptivePanel } from '@/shared/ui/adaptive-panel'
import { Input } from '@/shared/ui/input'

/**
 * Choosing one item: a search field, then the list in the default slot. A bottom sheet on a phone,
 * a popover under the trigger and at least as wide as it on a desktop (`AdaptivePanel`). The
 * trigger goes in the `trigger` slot.
 */
const props = defineProps<{ title: string; searchLabel: string; overlay: string }>()
const open = defineModel<boolean>('open', { default: false })
const search = defineModel<string>('search', { default: '' })
</script>

<template>
  <AdaptivePanel v-model:open="open" :title="props.title" :overlay="props.overlay" width="w-(--reka-popover-trigger-width) min-w-80">
    <template #trigger><slot name="trigger" /></template>
    <template #header>
      <Input v-model="search" type="search" :aria-label="props.searchLabel" :placeholder="props.searchLabel" autocomplete="off" />
    </template>
    <slot />
  </AdaptivePanel>
</template>
