<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import type { PeriodPreset } from '@/shared/lib/period'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { RANGE_PRESETS } from './range-presets'

/**
 * The range switch: 1M · 3M · 1Y · All as one pill track. It picks a quick
 * period; a period outside these four leaves it with nothing lit.
 */
defineProps<{ modelValue: PeriodPreset | '' }>()
const emit = defineEmits<{ 'update:modelValue': [PeriodPreset] }>()

const { t } = useI18n()

/** Clicking the lit segment would clear it; a range always stays picked. */
function pick(value: unknown) {
  if (typeof value === 'string' && value) emit('update:modelValue', value as PeriodPreset)
}
</script>

<template>
  <ToggleGroup type="single" variant="range" :spacing="1" data-range-switch :model-value="modelValue" @update:model-value="pick">
    <ToggleGroupItem v-for="preset in RANGE_PRESETS" :key="preset" :value="preset" class="flex-1" :data-range="preset">
      {{ t(`period.range.${preset}`) }}
    </ToggleGroupItem>
  </ToggleGroup>
</template>
