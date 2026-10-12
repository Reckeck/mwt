<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Check, X } from 'lucide-vue-next'

import { useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Operations workflows prepared after «Ждать подтверждения»: yes or no. */
const { t } = useI18n()
const actions = useWorkflowActions()
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const nameOf = (id: string) => (workflows.data.value ?? []).find((entry) => entry.id === id)?.name ?? id
const empty = computed(() => queue.isSuccess.value && !(queue.data.value ?? []).length)
</script>

<template>
  <div class="grid gap-2" data-tour="workflow-queue">
    <p class="text-sm text-muted-foreground">{{ t('workflow.queue.intro') }}</p>
    <div v-if="empty" class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
      {{ t('workflow.queue.empty') }}
    </div>
    <Card v-for="batch in queue.data.value ?? []" :key="batch.id" size="sm">
      <div class="flex flex-wrap items-start gap-3">
        <div class="grid min-w-0 flex-1 gap-1">
          <span class="font-medium">{{ batch.summary }}</span>
          <span class="text-xs text-muted-foreground">
            {{ nameOf(batch.workflow_id) }} · {{ formatDateTime(batch.created_at) }} ·
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
  </div>
</template>
