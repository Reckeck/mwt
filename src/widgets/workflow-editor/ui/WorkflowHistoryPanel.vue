<script setup lang="ts">
// check-ui-ignore control-swap: restore asks first only while the editor has unsaved edits
import { useI18n } from 'vue-i18n'
import { History } from 'lucide-vue-next'

import { useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { formatDateTime } from '@/shared/lib/format'
import { parseGraph } from '@/shared/lib/workflow'

/** A saved workflow's history: earlier versions to put back. Its tests live in `Check`. */
const props = defineProps<{ workflow: Workflow; dirty: boolean }>()
const emit = defineEmits<{ restored: [workflow: Workflow] }>()

const { t } = useI18n()
const actions = useWorkflowActions()

const versions = () => [...props.workflow.versions].reverse()
const nodeCount = (graph: string) => parseGraph(graph).nodes.length

function restore(savedAt: string) {
  actions.restoreVersion.mutate(
    { id: props.workflow.id, saved_at: savedAt },
    { onSuccess: (saved) => emit('restored', saved.workflow) },
  )
}
</script>

<template>
  <div class="grid gap-4">
    <section class="grid gap-1.5" data-tour="workflow-versions">
      <h4 class="flex items-center gap-1.5 text-sm font-medium">
        <History class="size-4" />
        {{ t('workflow.versions.title') }}
      </h4>
      <p v-if="!props.workflow.versions.length" class="text-xs text-muted-foreground">{{ t('workflow.versions.empty') }}</p>
      <ul class="grid gap-1">
        <li v-for="version in versions()" :key="version.saved_at" class="flex items-center gap-2 text-sm">
          <span class="min-w-0 flex-1">
            {{ formatDateTime(version.saved_at) }}
            <span class="text-xs text-muted-foreground">· {{ t('workflow.versions.nodes', { count: nodeCount(version.graph) }) }}</span>
          </span>
          <!-- Over unsaved edits a restore asks once more: they would be lost. -->
          <Button
            v-if="!props.dirty"
            size="sm"
            variant="ghost"
            :disabled="actions.restoreVersion.isPending.value"
            @click="restore(version.saved_at)"
          >
            {{ t('workflow.versions.restore') }}
          </Button>
          <ConfirmButton
            v-else
            :label="t('workflow.versions.restore')"
            :confirm-label="t('workflow.versions.restoreOverEdits')"
            :action-label="t('workflow.versions.restore')"
            :disabled="actions.restoreVersion.isPending.value"
            @confirm="restore(version.saved_at)"
          >
            <History class="size-4" />
          </ConfirmButton>
        </li>
      </ul>
    </section>
  </div>
</template>
