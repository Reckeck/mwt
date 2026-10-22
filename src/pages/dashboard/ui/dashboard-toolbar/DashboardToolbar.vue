<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { LayoutGrid, RotateCcw } from 'lucide-vue-next'

import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

/** The row above the grid: the shared period, «Сбросить раскладку» and the widget catalog. */
const props = defineProps<{ period: DashPeriod; hasMore: boolean }>()
const emit = defineEmits<{ reset: []; catalog: [] }>()
const { t } = useI18n()

/** One period for every widget that follows it; a widget may keep its own (its menu). */
const PERIODS: DashPeriod[] = ['month', 'quarter', 'year']
const settingsStore = useSettingsStore()
function choosePeriod(value: unknown) {
  // A single toggle group hands back `undefined` when the pressed item is clicked again.
  if (typeof value !== 'string' || value === props.period) return
  settingsStore
    .updateVault({ dashboard_period: value as DashPeriod })
    .catch((err) => notify.error(err, 'settings.saveFailed'))
}
</script>

<template>
  <!-- The row is always there: the button appearing once the wallets load must not push the grid down. -->
  <div class="mb-3 flex items-center justify-between gap-2">
    <ToggleGroup
      :model-value="props.period"
      type="single"
      variant="outline"
      size="sm"
      :aria-label="t('dashboard.period.label')"
      data-tour="dashboard-period"
      @update:model-value="choosePeriod"
    >
      <ToggleGroupItem v-for="period in PERIODS" :key="period" :value="period" class="px-2.5 text-xs">
        {{ t(`dashboard.period.${period}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
    <div class="flex items-center gap-2">
      <Button
        variant="ghost"
        size="sm"
        data-tour="dash-reset"
        :title="t('dashboard.resetHint')"
        @click="emit('reset')"
      >
        <RotateCcw class="size-4" />
        <span class="max-sm:sr-only">{{ t('dashboard.reset') }}</span>
      </Button>
      <Button
        variant="outline"
        size="sm"
        data-tour="dashboard-add"
        :class="!props.hasMore && 'invisible'"
        :aria-hidden="!props.hasMore"
        :tabindex="props.hasMore ? undefined : -1"
        @click="emit('catalog')"
      >
        <LayoutGrid class="size-4" />
        {{ t('dashboard.catalog.button') }}
      </Button>
    </div>
  </div>
</template>
