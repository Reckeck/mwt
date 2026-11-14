<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RouterLink, useRouter } from 'vue-router'
import { ChevronDown, FileInput, Plus } from 'lucide-vue-next'

import { LessonList, WORKFLOW_TEMPLATES } from '@/entities/workflow'
import { WorkflowImportDialog, useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import { WorkflowCard } from './ui/workflow-card'

/** Every workflow: on or off, its last run, open, run, share, delete; and where to start. */
const { t } = useI18n()
const router = useRouter()
const actions = useWorkflowActions()

const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })
const runs = useQuery({
  queryKey: WORKFLOW_KEYS.runs,
  queryFn: () => call<WorkflowRun[]>('list_workflow_runs', { input: { workflow_id: null, limit: 200 } }),
})
/** The last real run, or none. */
const lastRun = (id: string) => (runs.data.value ?? []).find((run) => run.workflow_id === id && !run.dry_run) ?? null
const empty = computed(() => workflows.isSuccess.value && !(workflows.data.value ?? []).length)
const importOpen = ref(false)
/** Folded once the user has workflows of their own; open for a first visit. */
const learnOpen = ref(false)
watch(
  () => workflows.isSuccess.value,
  (loaded) => {
    if (loaded) learnOpen.value = empty.value
  },
  { immediate: true },
)
</script>

<template>
  <div class="grid gap-4">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="max-w-2xl text-sm text-muted-foreground">{{ t('workflow.list.intro') }}</p>
      <div class="flex gap-2">
        <Button variant="outline" data-workflow-import @click="importOpen = true">
          <FileInput class="size-4" />
          {{ t('workflow.list.import') }}
        </Button>
        <Button data-tour="workflow-new" @click="router.push({ name: 'workflow-new' })">
          <Plus class="size-4" />
          {{ t('workflow.list.new') }}
        </Button>
      </div>
    </div>

    <h3 class="text-sm font-medium">{{ t('workflow.list.mine') }}</h3>
    <!-- The skeleton holds the cards' place: `How it works` below does not jump when they land. -->
    <SectionState
      :loading="workflows.isPending.value"
      :error="workflows.error.value"
      :empty="empty"
      :empty-title="t('workflow.list.empty')"
      :rows="2"
      compact
      @retry="workflows.refetch()"
    >
      <div class="grid gap-4">
        <WorkflowCard
          v-for="workflow in workflows.data.value ?? []"
          :key="workflow.id"
          :workflow="workflow"
          :last-run="lastRun(workflow.id)"
          :switching="actions.setActive.isPending.value"
          :running="actions.runNow.isPending.value"
          @active="(active) => actions.setActive.mutate({ id: workflow.id, active })"
          @edit="router.push({ name: 'workflow-edit', params: { id: workflow.id } })"
          @run="actions.runNow.mutate({ id: workflow.id })"
          @export="actions.exportFile.mutate(workflow.id)"
          @remove="actions.remove.mutate(workflow.id)"
        />

        <!-- Lessons and templates are for starting out: open while there is nothing of one's own. -->
      </div>
    </SectionState>

    <!-- After the cards: landing above it, they would push it down. -->
    <Collapsible v-if="!workflows.isPending.value" v-model:open="learnOpen" class="grid gap-3 rounded-lg border border-border p-3">
      <CollapsibleTrigger
        class="flex w-full items-center justify-between gap-2 text-left text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        {{ t('workflow.list.learn') }}
        <ChevronDown class="size-4 shrink-0 text-muted-foreground transition-transform" :class="learnOpen && 'rotate-180'" />
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-4">
        <LessonList />

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
      </CollapsibleContent>
    </Collapsible>

    <WorkflowImportDialog v-model:open="importOpen" @imported="(workflow) => router.push({ name: 'workflow-edit', params: { id: workflow.id } })" />
  </div>
</template>
