<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { DashboardAllocation } from '@/widgets/dashboard-allocation'
import { DashboardAssets } from '@/widgets/dashboard-assets'
import { DashboardFx } from '@/widgets/dashboard-fx'
import { DashboardTotal } from '@/widgets/dashboard-total'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settingsStore = useSettingsStore()

// Dashboard widget definitions matching backend
const widgetDefs = [
  { id: 'total', defaultOrder: 0 },
  { id: 'assets', defaultOrder: 1 },
  { id: 'fx', defaultOrder: 2 },
  { id: 'byChain', defaultOrder: 3 },
  { id: 'byStore', defaultOrder: 4 },
] as const

const visibleWidgets = computed(() => {
  const widgets = settingsStore.vault?.dashboard_widgets ?? []
  return widgetDefs
    .map((def) => {
      const existing = widgets.find((widget) => widget.id === def.id)
      return {
        id: def.id,
        visible: existing?.visible ?? true,
        order: existing?.order ?? def.defaultOrder,
      }
    })
    .filter((widget) => widget.visible)
    .sort((left, right) => left.order - right.order)
})

const isVisible = (id: (typeof widgetDefs)[number]['id']) => visibleWidgets.value.some((widget) => widget.id === id)
</script>

<template>
  <div class="grid gap-6 @container">
    <!-- Top row (total, assets, fx): 3 columns on lg, 2 on sm -->
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <DashboardTotal v-if="isVisible('total')" />
      <DashboardAssets v-if="isVisible('assets')" />
      <DashboardFx v-if="isVisible('fx')" />
    </div>

    <!-- Bottom row (by chain, by store): 2 columns on sm -->
    <div class="grid gap-4 @sm:grid-cols-2">
      <DashboardAllocation v-if="isVisible('byChain')" by="chain" />
      <DashboardAllocation v-if="isVisible('byStore')" by="store" />
    </div>

    <!-- Empty state when no widgets are visible -->
    <div v-if="!visibleWidgets.length" class="py-12 text-center text-sm text-muted-foreground">
      {{ t('store.empty') }}
    </div>
  </div>
</template>
