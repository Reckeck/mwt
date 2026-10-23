<script setup lang="ts">
import { computed } from 'vue'
import { RouterView } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { RouteTabs } from '@/shared/ui/route-tabs'

const { t } = useI18n()

/** Each tab is a child route, so a tab has its own address. */
const tabs = computed(() => [
  { name: 'transactions', label: t('transactions.tab.list') },
  { name: 'transactions-calendar', label: t('transactions.tab.calendar') },
])
/** Used less often: under «Ещё ▾». */
const more = computed(() => [
  { name: 'transactions-contacts', label: t('transactions.tab.contacts') },
  { name: 'transactions-bonuses', label: t('transactions.tab.bonuses') },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex min-w-0 flex-wrap items-center gap-2">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" /></div>
      <!-- A tab's own controls on the tabs' row (the calendar's month), teleported in. -->
      <div id="tx-section-end" class="ml-auto flex min-w-0 items-center" />
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
