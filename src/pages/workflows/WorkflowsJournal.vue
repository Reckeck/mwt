<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { WorkflowQueue } from '@/widgets/workflow-queue'
import { WorkflowRuns } from '@/widgets/workflow-runs'
import { buttonVariants } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

const { t } = useI18n()

// The same queries the sections read: a journal with nothing at all says so once.
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const waits = useQuery({ queryKey: WORKFLOW_KEYS.waits, queryFn: () => call<WorkflowWait[]>('list_workflow_waits') })
const runs = useQuery({
  queryKey: WORKFLOW_KEYS.runs,
  queryFn: () => call<WorkflowRun[]>('list_workflow_runs', { input: { workflow_id: null, limit: 200 } }),
})
const nothing = computed(
  () =>
    queue.isSuccess.value &&
    waits.isSuccess.value &&
    runs.isSuccess.value &&
    !queue.data.value?.length &&
    !waits.data.value?.length &&
    !runs.data.value?.length,
)
</script>

<template>
  <EmptyState v-if="nothing" :title="t('workflow.journal.empty')" :description="t('workflow.journal.emptyHint')">
    <RouterLink :to="{ name: 'workflows' }" :class="buttonVariants({ variant: 'outline' })">
      {{ t('workflow.journal.toLessons') }}
    </RouterLink>
  </EmptyState>
  <!-- What waits for you first, then what already ran. -->
  <div v-else class="grid gap-6">
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('workflow.journal.waiting') }}</h2>
      <WorkflowQueue />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('workflow.journal.history') }}</h2>
      <WorkflowRuns />
    </section>
  </div>
</template>
