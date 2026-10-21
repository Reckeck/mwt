<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { Gift, Plus } from 'lucide-vue-next'

import { PointsDialog, ProgramCreateDialog } from '@/features/loyalty-edit'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { useContacts, useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'

import ProgramCard from './ui/ProgramCard.vue'

/** Bonus programmes (plan G1): not wallets, but points with a worth, a login and the cards that earn them. */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const tree = useStoreTree()
const contacts = useContacts()
const report = useQuery({
  queryKey: ['cashback', 'year'],
  placeholderData: keepPreviousData,
  queryFn: () => call<CashbackReport>('get_cashback_report', { period: 'year' }),
})

const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
const programs = computed(() =>
  nodes.value.filter((node) => node.store.kind === 'loyalty' && !node.store.archived).map((node) => node.store),
)
const names = computed(() => new Map(nodes.value.map((node) => [node.store.id, node.store.name])))
const contactNames = computed(() => new Map((contacts.data.value ?? []).map((contact) => [contact.id, contact.name])))
const balanceOf = (id: string) => report.data.value?.programmes.find((row) => row.store_id === id) ?? null

/** `?program=<id>`: an old wallet link to a programme; its card is marked and scrolled to. */
const highlighted = computed(() => (typeof route.query.program === 'string' ? route.query.program : ''))
watch(
  [highlighted, () => programs.value.length],
  async ([id]) => {
    if (!id) return
    await nextTick()
    document.querySelector(`[data-program="${CSS.escape(id)}"]`)?.scrollIntoView({ block: 'center' })
  },
  { immediate: true },
)

const createOpen = ref(false)
const points = ref<{ store: Store; mode: 'credit' | 'spend' } | null>(null)
const pointsOpen = computed({
  get: () => points.value !== null,
  set: (value: boolean) => {
    if (!value) points.value = null
  },
})
function openSettings(store: Store) {
  void router.push({ name: 'wallet-settings', params: { id: store.id }, query: { tab: 'loyalty' } })
}
</script>

<template>
  <section class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <div class="grid gap-0.5">
        <h2 class="font-medium">{{ t('bonuses.title') }}</h2>
        <p class="text-sm text-muted-foreground">{{ t('bonuses.hint') }}</p>
      </div>
      <Button size="sm" data-tour="bonuses-add" @click="createOpen = true">
        <Plus class="size-4" />{{ t('bonuses.add') }}
      </Button>
    </div>
    <SectionState
      :loading="tree.isPending.value"
      :error="tree.error.value"
      skeleton="list"
      :rows="2"
      @retry="tree.refetch()"
    >
      <EmptyState v-if="!programs.length" :icon="Gift" :title="t('bonuses.emptyTitle')" :description="t('bonuses.emptyHint')">
        <Button size="sm" @click="createOpen = true"><Plus class="size-4" />{{ t('bonuses.add') }}</Button>
      </EmptyState>
      <div v-else class="grid gap-3 md:grid-cols-2" data-tour="bonuses-list">
        <ProgramCard
          v-for="store in programs"
          :key="store.id"
          :store="store"
          :balance="balanceOf(store.id)"
          :base-currency="report.data.value?.base_currency ?? 'usd'"
          :names="names"
          :contact-name="store.contact_id ? (contactNames.get(store.contact_id) ?? null) : null"
          :highlighted="store.id === highlighted"
          @points="(mode) => (points = { store, mode })"
          @settings="openSettings(store)"
        />
      </div>
    </SectionState>
    <ProgramCreateDialog v-model:open="createOpen" />
    <PointsDialog v-if="points" v-model:open="pointsOpen" :store="points.store" :mode="points.mode" />
  </section>
</template>
