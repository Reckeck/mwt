<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { MINE_FILTERS, type MineFilter, isMineFilter } from '../../lib/mine-view'

/** Все · Повторы · Схемы · Напоминания · Алерты (Auto-1 · 1): one is always on. */
const filter = defineModel<MineFilter>({ required: true })
const { t } = useI18n()

function pick(value: unknown) {
  if (isMineFilter(value)) filter.value = value
}
/** The chosen chip is filled with the text colour, as on Auto-1 · 1. */
const ON = 'data-[state=on]:border-foreground data-[state=on]:bg-foreground data-[state=on]:text-background'
</script>

<template>
  <ToggleGroup
    :model-value="filter"
    type="single"
    class="flex flex-wrap gap-1.5"
    :aria-label="t('automations.mine.filter')"
    data-mine-chips
    @update:model-value="pick"
  >
    <ToggleGroupItem
      v-for="entry in MINE_FILTERS"
      :key="entry"
      :value="entry"
      class="h-7 rounded-md border border-border px-2.5 text-caption font-normal text-muted-foreground hover:text-foreground"
      :class="ON"
    >
      {{ t(`automations.mine.type.${entry}`) }}
    </ToggleGroupItem>
  </ToggleGroup>
</template>
