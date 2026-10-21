<script setup lang="ts">
import { computed } from 'vue'
import { RouterView } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { RouteTabs } from '@/shared/ui/route-tabs'

const { t } = useI18n()

/** Each tab is a child route, so a tab has its own address. */
const tabs = computed(() =>
  ['automations', 'sources', 'budgets', 'goals', 'calculator', 'reminders', 'alerts'].map((key) => ({
    name: `planning-${key}`,
    label: t(`planning.tab.${key}`),
  })),
)
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" /></div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
