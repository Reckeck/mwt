<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Variable } from 'lucide-vue-next'

import { WorkflowVars } from '@/widgets/workflow-vars'
import { Button } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { RouteTabs } from '@/shared/ui/route-tabs'
import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** The journal's tab shows how many wait for you: questions, and operations left from an older app. */
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const waits = useQuery({ queryKey: WORKFLOW_KEYS.waits, queryFn: () => call<WorkflowWait[]>('list_workflow_waits') })
const waiting = computed(
  () => (waits.data.value ?? []).filter((wait) => wait.kind === 'confirm').length + (queue.data.value?.length ?? 0),
)

const tabs = computed(() => [
  { name: 'workflows', label: t('workflow.tab.list') },
  {
    name: 'workflows-journal',
    label: waiting.value ? `${t('workflow.tab.journal')} (${waiting.value})` : t('workflow.tab.journal'),
  },
])
/** Variables open over any tab; `?vars=1` keeps the panel across Back and a reload. */
const varsOpen = computed({
  get: () => route.query.vars === '1',
  set: (open: boolean) => void router.replace({ query: { ...route.query, vars: open ? '1' : undefined } }),
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" /></div>
      <Button variant="outline" size="sm" data-tour="workflow-vars-open" @click="varsOpen = true">
        <Variable class="size-4" />
        {{ t('workflow.tab.vars') }}
      </Button>
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
    <Sheet v-model:open="varsOpen">
      <SheetContent side="right" class="w-full overflow-y-auto sm:max-w-2xl">
        <SheetHeader>
          <SheetTitle>{{ t('workflow.tab.vars') }}</SheetTitle>
          <SheetDescription>{{ t('workflow.vars.sheetHint') }}</SheetDescription>
        </SheetHeader>
        <div class="px-4 pb-4"><WorkflowVars /></div>
      </SheetContent>
    </Sheet>
  </div>
</template>
