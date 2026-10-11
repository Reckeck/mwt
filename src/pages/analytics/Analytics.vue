<script setup lang="ts">
import { computed, reactive } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { ReportExportButton } from '@/features/report-export'
import { ReportFilterBar } from '@/widgets/report-filter'
import { Tabs } from '@/shared/ui/tabs'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const KINDS: ReportKind[] = [
  'cashflow',
  'year',
  'categories',
  'networth',
  'stores',
  'losses',
  'pnl',
  'portfolio',
  'inflation',
]

const tabs = computed(() => KINDS.map((kind) => ({ value: `analytics-${kind}`, label: t(`analytics.tab.${kind}`) })))
const tab = computed({
  get: () => String(route.name ?? 'analytics-cashflow'),
  set: (name: string) => void router.push({ name }),
})
const kind = computed(() => (route.meta.report as ReportKind | undefined) ?? 'cashflow')

/** One filter for every tab, so switching tabs keeps the period and wallets. */
const filter = reactive<ReportFilter>({
  scope: { store_ids: [], include_children: true },
  from: null,
  to: null,
  year: null,
  currency: null,
  real_terms: false,
})
</script>

<template>
  <div class="grid gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <Tabs v-model="tab" :tabs="tabs" />
      <ReportExportButton :kind="kind" :filter="filter" />
    </div>
    <ReportFilterBar v-model="filter" :kind="kind" />
    <RouterView v-slot="{ Component }">
      <component :is="Component" :kind="kind" :filter="filter" />
    </RouterView>
  </div>
</template>
