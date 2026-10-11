<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { DASHBOARD_WIDGETS, type DashboardWidgetDef } from '@/widgets/dashboard-registry'
import { Button } from '@/shared/ui/button'
import { Select } from '@/shared/ui/select'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'

import type { DashboardCatalogProps } from './dashboard-catalog.types'

const props = defineProps<DashboardCatalogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ add: [def: DashboardWidgetDef, storeId: string | null] }>()
const { t } = useI18n()

const available = computed(() =>
  DASHBOARD_WIDGETS.filter((def) => !props.placed.includes(def.id) && (!def.requires || props.exchanges.length)),
)

/** Which exchange a trading widget will show; the first one until the user picks another. */
const exchangeId = ref('')
watch(
  () => props.exchanges,
  (list) => {
    if (!list.some((exchange) => exchange.id === exchangeId.value)) exchangeId.value = list[0]?.id ?? ''
  },
  { immediate: true },
)

function widgetProps(def: DashboardWidgetDef): Record<string, unknown> {
  return def.requires ? { ...def.props, storeId: exchangeId.value } : (def.props ?? {})
}

function add(def: DashboardWidgetDef) {
  emit('add', def, def.requires ? exchangeId.value : null)
}
</script>

<template>
  <Sheet v-model:open="open">
    <SheetContent side="right" class="w-full gap-0 sm:max-w-md">
      <SheetHeader class="border-b border-border">
        <SheetTitle>{{ t('dashboard.catalog.title') }}</SheetTitle>
        <SheetDescription>{{ t('dashboard.catalog.description') }}</SheetDescription>
      </SheetHeader>
      <div class="min-h-0 flex-1 overflow-y-auto p-4">
        <p v-if="!available.length" class="text-sm text-muted-foreground">{{ t('dashboard.catalog.allPlaced') }}</p>
        <ul class="grid gap-4">
          <li v-for="def in available" :key="def.id" class="grid gap-2 rounded-lg border border-border p-3">
            <div class="flex items-start justify-between gap-3">
              <div class="grid gap-0.5">
                <p class="text-sm font-medium">{{ t(def.titleKey) }}</p>
                <p class="text-xs text-muted-foreground">{{ t(def.descriptionKey) }}</p>
              </div>
              <Button size="sm" variant="outline" @click="add(def)">
                <Plus class="size-4" />
                {{ t('dashboard.catalog.add') }}
              </Button>
            </div>
            <Select
              v-if="def.requires && exchanges.length > 1"
              v-model="exchangeId"
              class="h-8"
              :aria-label="t('dashboard.catalog.exchange')"
            >
              <option v-for="exchange in exchanges" :key="exchange.id" :value="exchange.id">{{ exchange.name }}</option>
            </Select>
            <!-- A live, scaled-down copy of the widget: no screenshots to keep in sync. -->
            <div class="pointer-events-none relative h-40 overflow-hidden rounded-md border border-dashed border-border bg-muted/30" aria-hidden="true" inert>
              <div class="@container absolute left-0 top-0 w-[200%] origin-top-left scale-50">
                <component :is="def.component" v-bind="widgetProps(def)" />
              </div>
            </div>
          </li>
        </ul>
      </div>
    </SheetContent>
  </Sheet>
</template>
