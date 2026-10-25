<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { DASHBOARD_WIDGETS, type DashboardWidgetDef } from '@/widgets/dashboard-registry'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Select, SelectItem } from '@/shared/ui/select'

import type { DashboardCatalogProps } from './dashboard-catalog.types'
import CatalogOption from './CatalogOption.vue'

/**
 * «Виджеты на Главной» (Home 4): each widget is a card that is on or off.
 * The few made for Home come first; the ones that repeat «Деньги» wait folded.
 */
const props = defineProps<DashboardCatalogProps>()
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ add: [def: DashboardWidgetDef, storeId: string | null]; remove: [id: string] }>()
const { t } = useI18n()

const offered = computed(() => DASHBOARD_WIDGETS.filter((def) => !def.requires || props.exchanges.length))
const main = computed(() => offered.value.filter((def) => def.home))
const more = computed(() => offered.value.filter((def) => !def.home))
const isOn = (def: DashboardWidgetDef) => props.placed.includes(def.id)
/** «Ещё» opens by itself when one of its widgets is already on Home. */
const moreOpen = computed(() => more.value.some(isOn))

/** Which exchange a trading widget will show; the first one until the user picks another. */
const exchangeId = ref('')
watch(
  () => props.exchanges,
  (list) => {
    if (!list.some((exchange) => exchange.id === exchangeId.value)) exchangeId.value = list[0]?.id ?? ''
  },
  { immediate: true },
)

function toggle(def: DashboardWidgetDef) {
  if (isOn(def)) emit('remove', def.id)
  else emit('add', def, def.requires ? exchangeId.value : null)
}
</script>

<template>
  <Dialog overlay="dashboard-catalog" v-model:open="open" :title="t('dashboard.catalog.title')" :description="t('dashboard.catalog.description')">
    <div class="grid gap-4">
      <div class="grid gap-2.5 sm:grid-cols-2">
        <CatalogOption v-for="def in main" :key="def.id" :def="def" :on="isOn(def)" @toggle="toggle(def)" />
      </div>
      <AdvancedFields v-if="more.length" :label="t('dashboard.catalog.more')" :default-open="moreOpen">
        <Select
          v-if="exchanges.length > 1"
          v-model="exchangeId"
          class="h-8 w-fit"
          :aria-label="t('dashboard.catalog.exchange')"
        >
          <SelectItem v-for="exchange in exchanges" :key="exchange.id" :value="exchange.id">{{ exchange.name }}</SelectItem>
        </Select>
        <div class="grid gap-2.5 sm:grid-cols-2">
          <CatalogOption v-for="def in more" :key="def.id" :def="def" :on="isOn(def)" @toggle="toggle(def)" />
        </div>
      </AdvancedFields>
      <p class="rounded-lg bg-muted px-3 py-2.5 text-sm">{{ t('dashboard.catalog.moneyNote') }}</p>
    </div>
    <template #footer>
      <Button @click="open = false">{{ t('dashboard.catalog.done') }}</Button>
    </template>
  </Dialog>
</template>
