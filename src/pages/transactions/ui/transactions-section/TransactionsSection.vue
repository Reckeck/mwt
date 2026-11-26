<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterView } from 'vue-router'

import { RouteTabs } from '@/shared/ui/route-tabs'

/**
 * «Operations»: the list, the spending calendar, bonuses and cashback, and the crypto module's
 * operations, one strip over the four routes (spec D4). The page under it brings its own filter and export.
 */
const { t } = useI18n()

const tabs = computed(() => [
  { name: 'transactions', label: t('transactions.tab.list') },
  { name: 'transactions-calendar', label: t('transactions.tab.calendar') },
  { name: 'transactions-bonuses', label: t('transactions.tab.bonuses') },
  { name: 'crypto-operations', label: t('cryptoTrack.operations.title') },
])
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-4 @container">
    <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" /></div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
