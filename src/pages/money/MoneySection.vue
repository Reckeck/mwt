<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterView, useRoute } from 'vue-router'

import { RouteTabs } from '@/shared/ui/route-tabs'

import { budgetTabs } from './model/money-groups'

/** The frame of the Budget section: the route tabs, then the screen of the open tab. */
const { t } = useI18n()
const route = useRoute()

/** The frame is the second-to-last matched record (the open screen is the last), its children are the tabs. */
const tabs = computed(() =>
  budgetTabs((route.matched.at(-2)?.children ?? []).map((child) => ({ name: child.name, meta: child.meta ?? {} }))).map((tab) => ({ name: tab.name, label: t(tab.labelKey) })),
)
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" /></div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
