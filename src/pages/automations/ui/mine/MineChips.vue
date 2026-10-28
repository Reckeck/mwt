<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { MINE_FILTERS, type MineFilter, isMineFilter } from '../../lib/mine-view'

/** Все · Повторы · Схемы · Напоминания · Алерты (Auto-1 · 1): one is always on; on a phone one 40 px row that scrolls. */
const filter = defineModel<MineFilter>({ required: true })
const { t } = useI18n()

function pick(value: unknown) {
  if (isMineFilter(value)) filter.value = value
}
/** The chosen chip is filled with the primary colour, as on Auto-1 · 1 (390, 2026-10-28). */
const ON = 'data-[state=on]:border-primary data-[state=on]:bg-primary data-[state=on]:text-primary-foreground'
</script>

<template>
  <ToggleGroup
    :model-value="filter"
    type="single"
    class="scrollbar-none flex min-w-0 gap-1.5 overflow-x-auto md:flex-wrap"
    :aria-label="t('automations.mine.filter')"
    data-mine-chips
    @update:model-value="pick"
  >
    <ToggleGroupItem
      v-for="entry in MINE_FILTERS"
      :key="entry"
      :value="entry"
      class="h-7 shrink-0 rounded-md border border-border px-2.5 text-caption font-normal text-muted-foreground hover:text-foreground max-md:h-10 max-md:px-3 max-md:text-sm"
      :class="ON"
    >
      {{ t(`automations.mine.type.${entry}`) }}
    </ToggleGroupItem>
  </ToggleGroup>
</template>
