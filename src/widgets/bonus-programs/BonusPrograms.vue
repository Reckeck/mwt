<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Gift, Plus } from 'lucide-vue-next'

import { PointsDialog, ProgramCreateDialog } from '@/features/loyalty-edit'
import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { ActionSheet } from '@/shared/ui/action-menu'
import type { UiAction } from '@/shared/ui/context-menu'
import { SectionState } from '@/shared/ui/section-state'
import { useIsPhone } from '@/shared/lib/phone'

import type { PointsMode } from './bonus-programs.types'
import { useProgramRows } from './model/use-program-rows'
import ProgramList from './ui/ProgramList.vue'
import ProgramTable from './ui/ProgramTable.vue'

/**
 * Bonuses (Ops-2 · 12): what they come to in the base currency, then the
 * programmes — a table on a desktop, a list on a phone. «+ Программа» sits on
 * the tabs' row when the page gives a place for it (`addTo`).
 */
const props = defineProps<{ addTo?: string }>()
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()

const { tree, rows, total, baseCurrency } = useProgramRows()

/** `?program=<id>`: an old wallet link to a programme; its row is marked and scrolled to. */
const highlighted = computed(() => (typeof route.query.program === 'string' ? route.query.program : ''))
watch(
  [highlighted, () => rows.value.length],
  async ([id]) => {
    if (!id) return
    await nextTick()
    document.querySelector(`[data-program="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'center' })
  },
  { immediate: true },
)

const createOpen = ref(false)
/** `?new=program`: the wallet wizard handed over «Лояльность и бонусы»; the query is dropped once open. */
watch(
  () => route.query.new,
  (value) => {
    if (value !== 'program') return
    createOpen.value = true
    void router.replace({ query: { ...route.query, new: undefined } })
  },
  { immediate: true },
)
const points = ref<{ store: Store; mode: PointsMode } | null>(null)
const pointsOpen = computed({
  get: () => points.value !== null,
  set: (value: boolean) => {
    if (!value) points.value = null
  },
})
/** «Начислить» on a phone: straight to the one programme that counts points, else a pick of them. */
const creditable = computed(() => rows.value.filter((row) => row.store.loyalty))
const pickOpen = ref(false)
const creditActions = computed<UiAction[]>(() =>
  creditable.value.map((row) => ({ id: row.store.id, label: row.store.name, hint: row.points, run: () => credit(row.store) })),
)
function credit(store: Store) {
  points.value = { store, mode: 'credit' }
}
function creditAny() {
  const [only, ...more] = creditable.value
  if (only && !more.length) credit(only.store)
  else pickOpen.value = true
}
function openSettings(store: Store) {
  void router.push({ name: 'wallet-settings', params: { id: store.id }, query: { tab: 'loyalty' } })
}
</script>

<template>
  <Teleport v-if="props.addTo && !phone" defer :to="props.addTo">
    <Button variant="outline" data-tour="bonuses-add" @click="createOpen = true">
      <Plus class="size-4" />{{ t('bonuses.addShort') }}
    </Button>
  </Teleport>

  <section class="grid grid-cols-1 gap-4">
    <SectionState :loading="tree.isPending.value" :error="tree.error.value" skeleton="list" :rows="2" @retry="tree.refetch()">
      <EmptyState v-if="!rows.length" :icon="Gift" :title="t('bonuses.emptyTitle')" :description="t('bonuses.emptyHint')">
        <Button size="sm" @click="createOpen = true"><Plus class="size-4" />{{ t('bonuses.addShort') }}</Button>
      </EmptyState>
      <div v-else class="grid grid-cols-1 gap-4" data-tour="bonuses-list">
        <Card data-tour="bonuses-total">
          <div class="flex flex-col gap-1">
            <span class="text-xs text-muted-foreground">{{ t('bonuses.total') }}</span>
            <span class="text-2xl font-bold tabular-nums">≈ <Money :amount="total" :currency="baseCurrency" :decimals="0" /></span>
            <span class="text-xs text-muted-foreground">{{ t('bonuses.totalHint') }}</span>
          </div>
        </Card>
        <ProgramList
          v-if="phone"
          :rows="rows"
          :base-currency="baseCurrency"
          :highlighted="highlighted"
          @points="(store, mode) => (points = { store, mode })"
          @settings="openSettings"
        />
        <ProgramTable
          v-else
          :rows="rows"
          :base-currency="baseCurrency"
          :highlighted="highlighted"
          @points="(store, mode) => (points = { store, mode })"
          @settings="openSettings"
        />
      </div>
    </SectionState>
    <div v-if="phone && rows.length" class="grid grid-cols-2 gap-2">
      <Button variant="outline" class="h-11" :disabled="!creditable.length" data-bonuses-credit @click="creditAny">
        {{ t('loyalty.credit') }}
      </Button>
      <Button variant="outline" class="h-11" data-tour="bonuses-add" @click="createOpen = true">
        <Plus class="size-4" />{{ t('bonuses.addShort') }}
      </Button>
    </div>
    <ActionSheet v-if="phone" v-model:open="pickOpen" :actions="creditActions" :title="t('loyalty.credit')" />
    <ProgramCreateDialog v-model:open="createOpen" />
    <PointsDialog v-if="points" v-model:open="pointsOpen" :store="points.store" :mode="points.mode" />
  </section>
</template>
