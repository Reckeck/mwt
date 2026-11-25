<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { RouteTabs } from '@/shared/ui/route-tabs'
import { useIsPhone } from '@/shared/lib/phone'

const { t } = useI18n()
const route = useRoute()
const phone = useIsPhone()
/** On a phone the bonuses are a drill-down (Ops-2 · 12): no tabs, the top bar has Back. */
const inBonuses = computed(() => phone.value && route.name === 'transactions-bonuses')

/** Each tab is a child route, so a tab has its own address. */
const tabs = computed(() => [
  { name: 'transactions', label: t('transactions.tab.list') },
  { name: 'crypto-operations', label: t('cryptoTrack.operations.title') },
  { name: 'transactions-calendar', label: t('transactions.tab.calendar') },
  { name: 'transactions-bonuses', label: t('transactions.tab.bonuses') },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <!-- On a phone the bonuses are a drill-down: the top bar carries Back and the title. -->
    <div v-if="!inBonuses" class="flex min-w-0 flex-wrap items-center gap-2">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" /></div>
      <!-- A tab's own controls on the tabs' row (the calendar's month), teleported in. -->
      <div id="tx-section-end" class="ml-auto flex min-w-0 items-center" />
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
