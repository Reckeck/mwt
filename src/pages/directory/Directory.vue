<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Tabs } from '@/shared/ui/tabs'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** Each tab is a child route, so a tab has its own address. */
const tabs = computed(() => [
  { value: 'directory-contacts', label: t('directory.contacts') },
  { value: 'directory-rates', label: t('directory.rates') },
])

const tab = computed({
  get: () => String(route.name ?? 'directory-contacts'),
  set: (name: string) => void router.push({ name }),
})
</script>

<template>
  <div class="grid gap-4">
    <div class="w-fit max-w-full" data-tour="section-tabs"><Tabs v-model="tab" :tabs="tabs" /></div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
