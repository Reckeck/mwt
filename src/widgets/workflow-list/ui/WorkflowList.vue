<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { RouterLink, useRouter } from 'vue-router'
import { ref } from 'vue'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { FileInput, Pencil, Trash2, Workflow as WorkflowIcon } from 'lucide-vue-next'

import { LessonList } from '@/entities/workflow'
import { WorkflowImportDialog } from '@/features/workflow-manage'
import { TemplateGallery } from '@/features/workflow-template-gallery'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { ListRow } from '@/shared/ui/list-row'
import { SectionState } from '@/shared/ui/section-state'
import { Switch } from '@/shared/ui/switch'
import { buttonVariants } from '@/shared/ui/button'

/** The user's workflows: a switch, the way into the editor, delete. */
const { t } = useI18n()
const queryClient = useQueryClient()
const list = useQuery({
  queryKey: [...WORKFLOW_KEYS.list, 'all'],
  queryFn: () => call<Workflow[]>('list_workflows'),
  placeholderData: keepPreviousData,
})
const router = useRouter()
const importOpen = ref(false)
const refresh = () => void queryClient.invalidateQueries({ queryKey: [...WORKFLOW_KEYS.list] })

const setActive = useMutation({
  mutationFn: (input: { id: string; active: boolean }) => call<Workflow>('set_workflow_active', { input }),
  onSettled: refresh,
  meta: { error: 'automations.mine.toggleFailed', silent: 'the switch itself shows the new state' },
})
const remove = useMutation({
  mutationFn: (id: string) => call<null>('delete_workflow', { id }),
  onSuccess: refresh,
  meta: { error: 'reminder.deleteFailed', success: 'reminder.deleted' },
})
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('workflow.list.intro') }}</p>
      <div class="flex gap-2">
        <Button variant="outline" size="sm" data-workflow-import @click="importOpen = true">
          <FileInput class="size-4" />
          {{ t('workflow.list.import') }}
        </Button>
        <RouterLink :to="{ name: 'workflow-new' }" :class="buttonVariants({ size: 'sm' })" data-tour="workflow-new">
          {{ t('workflow.list.new') }}
        </RouterLink>
      </div>
    </div>
    <SectionState
      :loading="list.isPending.value"
      :error="list.error.value"
      :empty="!list.data.value?.length"
      :empty-title="t('workflow.list.empty')"
      :rows="3"
      @retry="list.refetch()"
    >
      <Card size="sm">
        <ul class="divide-y divide-border">
          <li v-for="workflow in list.data.value" :key="workflow.id" class="flex items-center gap-2" :data-workflow="workflow.id">
            <div class="min-w-0 flex-1">
              <ListRow
                :title="workflow.name"
                :subtitle="workflow.description || formatDateTime(workflow.updated_at)"
                :icon="WorkflowIcon"
                size="withControl"
              />
            </div>
            <Switch
              :model-value="workflow.active"
              :aria-label="t('automations.mine.toggle', { name: workflow.name })"
              @update:model-value="(active) => setActive.mutate({ id: workflow.id, active })"
            />
            <RouterLink
              :to="{ name: 'workflow-edit', params: { id: workflow.id } }"
              :class="buttonVariants({ variant: 'ghost', size: 'icon-sm' })"
              :aria-label="t('workflow.list.edit')"
            >
              <Pencil class="size-4" />
            </RouterLink>
            <ConfirmButton
              :label="t('workflow.list.delete')"
              :confirm-label="t('workflow.list.deleteConfirm')"
              @confirm="remove.mutate(workflow.id)"
            >
              <Trash2 class="size-4" />
            </ConfirmButton>
          </li>
        </ul>
      </Card>
    </SectionState>
    <TemplateGallery />
    <LessonList />
    <WorkflowImportDialog v-model:open="importOpen" @imported="(workflow) => router.push({ name: 'workflow-edit', params: { id: workflow.id } })" />
  </div>
</template>
