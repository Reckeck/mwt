<script setup lang="ts">
// check-ui-ignore control-swap: restore asks first only while the editor has unsaved edits
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleCheck, FlaskConical, History, Trash2, XCircle } from 'lucide-vue-next'

import { useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { formatDateTime } from '@/shared/lib/format'
import { parseGraph } from '@/shared/lib/workflow'

/**
 * A saved workflow's history and checks: earlier versions to put back, and
 * tests that emulate it and say whether it still does what it should.
 */
const props = defineProps<{ workflow: Workflow; dirty: boolean }>()
const emit = defineEmits<{ restored: [workflow: Workflow] }>()

const { t, locale } = useI18n()
const actions = useWorkflowActions()
const results = ref<WorkflowTestResult[]>([])

const versions = () => [...props.workflow.versions].reverse()
const nodeCount = (graph: string) => parseGraph(graph).nodes.length
const resultOf = (id: string) => results.value.find((result) => result.test_id === id)
const problems = (result: WorkflowTestResult) =>
  (locale.value.startsWith('ru') ? result.problems_ru : result.problems_en).join('; ')

function restore(savedAt: string) {
  actions.restoreVersion.mutate(
    { id: props.workflow.id, saved_at: savedAt },
    { onSuccess: (saved) => emit('restored', saved.workflow) },
  )
}

function runTests() {
  actions.runTests.mutate(props.workflow.id, { onSuccess: (list) => (results.value = list) })
}
</script>

<template>
  <div class="grid gap-4">
    <section class="grid gap-1.5" data-tour="workflow-tests">
      <div class="flex items-center justify-between gap-2">
        <h4 class="text-sm font-medium">{{ t('workflow.tests.title') }}</h4>
        <Button
          v-if="props.workflow.tests.length"
          size="sm"
          variant="outline"
          :disabled="actions.runTests.isPending.value"
          @click="runTests"
        >
          <FlaskConical class="size-4" />
          {{ t('workflow.tests.run') }}
        </Button>
      </div>
      <p class="text-xs text-muted-foreground">
        {{ props.workflow.tests.length ? t('workflow.tests.hint') : t('workflow.tests.empty') }}
      </p>
      <p v-if="props.dirty && props.workflow.tests.length" class="text-xs text-muted-foreground">
        {{ t('workflow.tests.savedOnly') }}
      </p>
      <ul class="grid gap-1">
        <li v-for="test in props.workflow.tests" :key="test.id" class="flex items-start gap-2 text-sm">
          <CircleCheck v-if="resultOf(test.id)?.passed" class="mt-0.5 size-4 shrink-0 text-primary" />
          <XCircle v-else-if="resultOf(test.id)" class="mt-0.5 size-4 shrink-0 text-destructive" />
          <span v-else class="mt-0.5 size-4 shrink-0 rounded-full border border-border" />
          <span class="min-w-0 flex-1">
            <span class="block">{{ test.name }}</span>
            <span v-if="resultOf(test.id) && !resultOf(test.id)?.passed" class="block text-xs text-destructive">
              {{ problems(resultOf(test.id) as WorkflowTestResult) }}
            </span>
          </span>
          <ConfirmButton
            :label="t('workflow.tests.delete')"
            :confirm-label="t('workflow.tests.deleteConfirm')"
            @confirm="actions.deleteTest.mutate({ workflow_id: props.workflow.id, test_id: test.id })"
          >
            <Trash2 class="size-4" />
          </ConfirmButton>
        </li>
      </ul>
    </section>

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
