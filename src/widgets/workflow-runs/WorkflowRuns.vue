<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { ChevronDown } from 'lucide-vue-next'

import { OutcomeBadge, RunTrace } from '@/entities/workflow'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** The journal: every run, newest first, with its steps in words. */
const { t } = useI18n()
const runs = useQuery({
  queryKey: WORKFLOW_KEYS.runs,
  queryFn: () => call<WorkflowRun[]>('list_workflow_runs', { input: { workflow_id: null, limit: 200 } }),
})
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const nameOf = (id: string) => (workflows.data.value ?? []).find((entry) => entry.id === id)?.name ?? id
const open = ref<string | null>(null)
const empty = computed(() => runs.isSuccess.value && !(runs.data.value ?? []).length)
</script>

<template>
  <div class="grid gap-2" data-tour="workflow-runs">
    <p class="text-sm text-muted-foreground">{{ t('workflow.runs.intro') }}</p>
    <div v-if="empty" class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
      {{ t('workflow.runs.empty') }}
    </div>
    <Card v-for="run in runs.data.value ?? []" :key="run.id" size="sm">
      <button
        type="button"
        class="flex w-full flex-wrap items-center gap-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :aria-expanded="open === run.id"
        @click="open = open === run.id ? null : run.id"
      >
        <OutcomeBadge :outcome="run.outcome" :dry-run="run.dry_run" />
        <span class="font-medium">{{ nameOf(run.workflow_id) }}</span>
        <span class="text-xs text-muted-foreground">{{ formatDateTime(run.started_at) }} · {{ run.trigger }}</span>
        <span v-if="run.created.length" class="text-xs text-muted-foreground">{{ t('workflow.run.made', { count: run.created.length }) }}</span>
        <span v-if="run.queued" class="text-xs text-muted-foreground">{{ t('workflow.run.queued', { count: run.queued }) }}</span>
        <span class="flex-1" />
        <ChevronDown class="size-4 transition-transform" :class="open === run.id ? 'rotate-180' : ''" />
      </button>
      <div v-if="open === run.id" class="mt-3"><RunTrace :steps="run.trace" /></div>
    </Card>
  </div>
</template>
