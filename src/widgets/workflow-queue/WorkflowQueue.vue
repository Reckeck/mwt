<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Check, Clock, MessageCircleQuestion, X } from 'lucide-vue-next'

import { useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'
import { EntityLink } from '@/shared/ui/entity-link'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/**
 * What workflows wait for: questions of `Wait for confirmation` (yes or no),
 * operations prepared earlier for confirmation, and runs asleep at `Wait`.
 */
const { t } = useI18n()
const actions = useWorkflowActions()
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const waits = useQuery({ queryKey: WORKFLOW_KEYS.waits, queryFn: () => call<WorkflowWait[]>('list_workflow_waits') })
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const nameOf = (id: string) => (workflows.data.value ?? []).find((entry) => entry.id === id)?.name ?? id

const questions = computed(() => (waits.data.value ?? []).filter((wait) => wait.kind === 'confirm'))
const asleep = computed(() => (waits.data.value ?? []).filter((wait) => wait.kind === 'timer'))
const batches = computed(() => queue.data.value ?? [])
const empty = computed(
  () => queue.isSuccess.value && waits.isSuccess.value && !batches.value.length && !(waits.data.value ?? []).length,
)
</script>

<template>
  <div class="grid gap-3" data-tour="workflow-queue">
    <p class="text-sm text-muted-foreground">{{ t('workflow.queue.intro') }}</p>
    <EmptyState v-if="empty" compact :title="t('workflow.queue.empty')" />

    <section v-if="questions.length" class="grid gap-2" data-tour="workflow-questions">
      <h3 class="text-sm font-semibold">{{ t('workflow.queue.questions') }}</h3>
      <p class="text-xs text-muted-foreground">{{ t('workflow.queue.questionsIntro') }}</p>
      <Card v-for="wait in questions" :key="wait.id" size="sm">
        <div class="flex flex-wrap items-start gap-3">
          <MessageCircleQuestion class="mt-0.5 size-4 shrink-0 text-primary" />
          <div class="grid min-w-0 flex-1 gap-1">
            <span class="font-medium">{{ wait.question || t('workflow.defaultQuestion') }}</span>
            <span class="text-xs text-muted-foreground">
              <EntityLink :target="{ kind: 'workflow', id: wait.workflow_id }" :label="nameOf(wait.workflow_id)" plain />
              <template v-if="wait.created_at"> · {{ formatDateTime(wait.created_at) }}</template>
              <template v-if="wait.resume_at"> · {{ t('workflow.queue.deadline', { when: formatDateTime(wait.resume_at) }) }}</template>
            </span>
          </div>
          <Button size="sm" :disabled="actions.answerWait.isPending.value" @click="actions.answerWait.mutate({ id: wait.id, yes: true })">
            <Check class="size-4" />
            {{ t('workflow.queue.yes') }}
          </Button>
          <Button
            size="sm"
            variant="outline"
            :disabled="actions.answerWait.isPending.value"
            @click="actions.answerWait.mutate({ id: wait.id, yes: false })"
          >
            <X class="size-4" />
            {{ t('workflow.queue.no') }}
          </Button>
        </div>
      </Card>
    </section>

    <section v-if="batches.length" class="grid gap-2">
      <h3 class="text-sm font-semibold">{{ t('workflow.queue.operations') }}</h3>
      <Card v-for="batch in batches" :key="batch.id" size="sm">
        <div class="flex flex-wrap items-start gap-3">
          <div class="grid min-w-0 flex-1 gap-1">
            <span class="font-medium">{{ batch.summary }}</span>
            <span class="text-xs text-muted-foreground">
              <EntityLink :target="{ kind: 'workflow', id: batch.workflow_id }" :label="nameOf(batch.workflow_id)" plain /> · {{ formatDateTime(batch.created_at) }} ·
              {{ t('workflow.queue.count', { count: batch.transactions.length }) }}
            </span>
          </div>
          <Button size="sm" :disabled="actions.confirmQueued.isPending.value" @click="actions.confirmQueued.mutate(batch.id)">
            <Check class="size-4" />
            {{ t('workflow.queue.confirm') }}
          </Button>
          <Button size="sm" variant="outline" :disabled="actions.discardQueued.isPending.value" @click="actions.discardQueued.mutate(batch.id)">
            <X class="size-4" />
            {{ t('workflow.queue.discard') }}
          </Button>
        </div>
      </Card>
    </section>

    <section v-if="asleep.length" class="grid gap-2">
      <h3 class="text-sm font-semibold">{{ t('workflow.queue.asleep') }}</h3>
      <Card v-for="wait in asleep" :key="wait.id" size="sm">
        <div class="flex items-center gap-3">
          <Clock class="size-4 shrink-0 text-muted-foreground" />
          <span class="min-w-0 flex-1 truncate text-sm"><EntityLink :target="{ kind: 'workflow', id: wait.workflow_id }" :label="nameOf(wait.workflow_id)" plain /></span>
          <span class="text-xs text-muted-foreground">{{ t('workflow.queue.asleepUntil', { when: formatDateTime(wait.resume_at) }) }}</span>
        </div>
      </Card>
    </section>
  </div>
</template>
