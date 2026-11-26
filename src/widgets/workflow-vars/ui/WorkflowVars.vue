<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'

import { WorkflowVarDialog, useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** Variables: shared ones every workflow reads, and each workflow's own. */
const { t } = useI18n()
const actions = useWorkflowActions()
const vars = useQuery({ queryKey: WORKFLOW_KEYS.vars, queryFn: () => call<WorkflowVar[]>('list_workflow_vars') })
const workflows = useQuery({ queryKey: WORKFLOW_KEYS.list, queryFn: () => call<Workflow[]>('list_workflows') })

const scopeText = (scope: VarScope) =>
  scope === 'global'
    ? t('workflow.vars.global')
    : ((workflows.data.value ?? []).find((entry) => entry.id === scope.workflow)?.name ?? t('workflow.vars.own'))

function valueText(raw: string): string {
  try {
    const value = JSON.parse(raw) as WfValue
    if (value.type === 'null') return '—'
    if (value.type === 'money') return `${value.value.amount} ${value.value.currency.toUpperCase()}`
    if (value.type === 'bool') return value.value ? t('workflow.vars.yes') : t('workflow.vars.no')
    if (typeof value.value === 'string' || typeof value.value === 'number') return String(value.value)
    return JSON.stringify(value.value)
  } catch {
    return raw
  }
}

const dialogOpen = ref(false)
const editing = ref<WorkflowVar | null>(null)
function open(variable: WorkflowVar | null) {
  editing.value = variable
  dialogOpen.value = true
}
const empty = computed(() => vars.isSuccess.value && !(vars.data.value ?? []).length)
</script>

<template>
  <div class="grid gap-4">
    <ListHeader :intro="t('workflow.vars.intro')" :action-label="t('workflow.vars.new')" :hide-action="empty" @action="open(null)" />
    <EmptyState v-if="empty" compact :title="t('workflow.vars.empty')" :description="t('workflow.vars.emptyHint')">
      <Button @click="open(null)"><Plus class="size-4" />{{ t('workflow.vars.new') }}</Button>
    </EmptyState>
    <Card v-for="variable in vars.data.value ?? []" :key="`${JSON.stringify(variable.scope)}:${variable.name}`" size="sm">
      <div class="flex flex-wrap items-center gap-3">
        <div class="grid min-w-0 flex-1 gap-0.5">
          <span class="font-medium">{{ variable.name }} = {{ valueText(variable.value) }}</span>
          <span class="text-xs text-muted-foreground">{{ scopeText(variable.scope) }} · {{ formatDateTime(variable.updated_at) }}</span>
        </div>
        <Button variant="ghost" size="icon" :aria-label="t('workflow.vars.edit')" @click="open(variable)"><Pencil class="size-4" /></Button>
        <ConfirmButton
          :label="t('workflow.vars.delete')"
          :confirm-label="t('actions.confirmDeleteNamed', { name: variable.name })"
          @confirm="actions.deleteVar.mutate({ scope: variable.scope, name: variable.name })"
        >
          <Trash2 class="size-4" />
        </ConfirmButton>
      </div>
    </Card>
    <WorkflowVarDialog v-model:open="dialogOpen" :variable="editing" />
  </div>
</template>
