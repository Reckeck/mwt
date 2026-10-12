<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RouterLink, useRouter } from 'vue-router'
import { Copy, FileInput, Pencil, Play, Plus, Trash2 } from 'lucide-vue-next'

import { OutcomeBadge, WORKFLOW_TEMPLATES } from '@/entities/workflow'
import { WorkflowImportDialog, useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Switch } from '@/shared/ui/switch'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Every workflow: on or off, its last run, open, run, share, delete; and where to start. */
const { t } = useI18n()
const router = useRouter()
const actions = useWorkflowActions()

const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const runs = useQuery({
  queryKey: WORKFLOW_KEYS.runs,
  queryFn: () => call<WorkflowRun[]>('list_workflow_runs', { input: { workflow_id: null, limit: 200 } }),
})
/** The last real run, as a list of none or one for the template. */
const lastRun = (id: string) =>
  (runs.data.value ?? []).filter((run) => run.workflow_id === id && !run.dry_run).slice(0, 1)
const empty = computed(() => workflows.isSuccess.value && !(workflows.data.value ?? []).length)
const importOpen = ref(false)
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="max-w-2xl text-sm text-muted-foreground">{{ t('workflow.list.intro') }}</p>
      <div class="flex gap-2">
        <Button variant="outline" @click="importOpen = true">
          <FileInput class="size-4" />
          {{ t('workflow.list.import') }}
        </Button>
        <Button data-tour="workflow-new" @click="router.push({ name: 'workflow-new' })">
          <Plus class="size-4" />
          {{ t('workflow.list.new') }}
        </Button>
      </div>
    </div>

    <section class="grid gap-2" data-tour="workflow-templates">
      <h3 class="text-sm font-medium">{{ t('workflow.list.templates') }}</h3>
      <div class="grid gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">
        <RouterLink
          v-for="template in WORKFLOW_TEMPLATES"
          :key="template.id"
          :to="{ name: 'workflow-new', query: { template: template.id } }"
          class="rounded-lg border border-border p-3 transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <span class="block text-sm font-medium">{{ t(`workflow.template.${template.id}.name`) }}</span>
          <span class="block text-xs text-muted-foreground">{{ t(`workflow.template.${template.id}.desc`) }}</span>
        </RouterLink>
      </div>
    </section>

    <div v-if="empty" class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
      {{ t('workflow.list.empty') }}
    </div>

    <Card v-for="workflow in workflows.data.value ?? []" :key="workflow.id" data-tour="workflow-card">
      <div class="flex flex-wrap items-start gap-3">
        <Switch
          :model-value="workflow.active"
          :aria-label="t('workflow.active')"
          :disabled="actions.setActive.isPending.value"
          @update:model-value="(active) => actions.setActive.mutate({ id: workflow.id, active })"
        />
        <div class="grid min-w-0 flex-1 gap-1">
          <RouterLink :to="{ name: 'workflow-edit', params: { id: workflow.id } }" class="font-medium hover:underline">
            {{ workflow.name }}
          </RouterLink>
          <p v-if="workflow.description" class="text-sm text-muted-foreground">{{ workflow.description }}</p>
          <p class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <template v-for="run in lastRun(workflow.id)" :key="run.id">
              <OutcomeBadge :outcome="run.outcome" />
              {{ formatDateTime(run.started_at) }}
            </template>
            <template v-if="!lastRun(workflow.id).length">{{ t('workflow.list.neverRan') }}</template>
          </p>
        </div>
        <div class="flex items-center gap-1">
          <Button variant="ghost" size="icon" :aria-label="t('workflow.list.edit')" @click="router.push({ name: 'workflow-edit', params: { id: workflow.id } })">
            <Pencil class="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            :aria-label="t('workflow.runNow')"
            :disabled="actions.runNow.isPending.value"
            @click="actions.runNow.mutate({ id: workflow.id })"
          >
            <Play class="size-4" />
          </Button>
          <Button variant="ghost" size="icon" :aria-label="t('workflow.list.export')" @click="actions.exportFile.mutate(workflow.id)">
            <Copy class="size-4" />
          </Button>
          <ConfirmButton :label="t('workflow.list.delete')" :confirm-label="t('workflow.list.deleteConfirm')" @confirm="actions.remove.mutate(workflow.id)">
            <Trash2 class="size-4" />
          </ConfirmButton>
        </div>
      </div>
    </Card>

    <WorkflowImportDialog v-model:open="importOpen" @imported="(workflow) => router.push({ name: 'workflow-edit', params: { id: workflow.id } })" />
  </div>
</template>
