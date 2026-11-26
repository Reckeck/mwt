<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterView } from 'vue-router'

import { RouteTabs } from '@/shared/ui/route-tabs'

import { GROUP_ROUTE, MAIN_GROUPS, MORE_GROUPS, type MoneyGroup } from './model/money-groups'

/** The frame of the Budget section: the route tabs, then the screen of the open tab. */
const { t } = useI18n()

const tab = (group: MoneyGroup, hint: boolean) => ({
  name: GROUP_ROUTE[group],
  label: t(`money.tab.${group}`),
  hint: hint ? t(`money.hint.${group}`) : undefined,
})
const tabs = computed(() => MAIN_GROUPS.map((group) => tab(group, false)))
const more = computed(() => MORE_GROUPS.map((group) => tab(group, true)))
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="w-fit max-w-full" data-tour="section-tabs">
      <!-- The query rides along, so the next tab keeps the month in the address. -->
      <RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" :more-title="t('money.moreTitle')" keep-query />
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
