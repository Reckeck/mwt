<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { ReportExportButton } from '@/features/report-export'
import { ReportFilterBar } from '@/widgets/report-filter'
import { Button } from '@/shared/ui/button'
import { Tabs } from '@/shared/ui/tabs'

import { useReportFilter } from './model/use-report-filter'

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
  // The filter rides along in the query, so the next tab shows the same period.
  set: (name: string) => void router.push({ name, query: route.query }),
})
const kind = computed(() => (route.meta.report as ReportKind | undefined) ?? 'cashflow')

const { filter, narrowed, reset } = useReportFilter()
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs"><Tabs v-model="tab" :tabs="tabs" /></div>
      <div data-tour="report-export"><ReportExportButton :kind="kind" :filter="filter" /></div>
    </div>
    <div class="flex flex-wrap items-end gap-3" data-tour="report-filter">
      <ReportFilterBar v-model="filter" class="min-w-0 flex-1" :kind="kind" />
      <Button v-if="narrowed" variant="ghost" @click="reset">
        <X class="size-4" />
        {{ t('filter.reset', { count: narrowed }) }}
      </Button>
    </div>
    <div class="min-w-0" data-tour="section-content">
      <RouterView v-slot="{ Component }">
        <component :is="Component" :kind="kind" :filter="filter" />
      </RouterView>
    </div>
  </div>
</template>
