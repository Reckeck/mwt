<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Tabs } from '@/shared/ui/tabs'
import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** The queue's tab shows how many wait for an answer. */
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })

const tabs = computed(() =>
  [
    ['workflows', 'list'],
    ['workflows-vars', 'vars'],
    ['workflows-queue', 'queue'],
    ['workflows-runs', 'runs'],
  ].map(([value, key]) => ({
    value,
    label:
      key === 'queue' && queue.data.value?.length
        ? `${t('workflow.tab.queue')} (${queue.data.value.length})`
        : t(`workflow.tab.${key}`),
  })),
)

const tab = computed({
  get: () => String(route.name ?? 'workflows'),
  set: (name: string) => void router.push({ name }),
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="w-fit max-w-full" data-tour="section-tabs"><Tabs v-model="tab" :tabs="tabs" /></div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
