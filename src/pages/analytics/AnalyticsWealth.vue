<script setup lang="ts">
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { AnalyticsReportView } from '@/widgets/analytics-reports'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'

const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const route = useRoute()
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="flex items-center gap-2" data-tour="real-terms" :title="t('analytics.realTermsHint')">
      <!-- The filter is the section's shared reactive state: the switch writes into it like the filter bar does. -->
      <Switch id="wealth-real-terms" v-model="props.filter.real_terms" />
      <Label for="wealth-real-terms">{{ t('analytics.realTerms') }}</Label>
      <RouterLink
        v-if="props.filter.real_terms"
        :to="{ query: { ...route.query, settings: 'inflation' } }"
        class="text-xs text-muted-foreground underline"
      >
        {{ t('analytics.realTermsRate') }}
      </RouterLink>
    </div>
    <AnalyticsReportView kind="networth" :filter="props.filter" />
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('report.byWallet') }}</h2>
      <AnalyticsReportView kind="stores" :filter="props.filter" hide-insights />
    </section>
  </div>
</template>
