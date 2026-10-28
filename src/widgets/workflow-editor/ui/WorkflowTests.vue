<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleCheck, FlaskConical, Trash2, XCircle } from 'lucide-vue-next'

import { useWorkflowActions } from '@/features/workflow-manage'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'

/**
 * A saved workflow's tests inside the «Проверка» block: each one emulates the
 * workflow and says whether it still does what it should.
 */
const props = defineProps<{ workflow: Workflow; dirty: boolean }>()

const { t, locale } = useI18n()
const actions = useWorkflowActions()
const results = ref<WorkflowTestResult[]>([])

/** Each test with its last run: passed, failed with the problems, or not run yet. */
const rows = computed(() =>
  props.workflow.tests.map((test) => {
    const result = results.value.find((item) => item.test_id === test.id)
    const problems = result && (locale.value.startsWith('ru') ? result.problems_ru : result.problems_en).join('; ')
    return { test, state: result ? (result.passed ? 'passed' : 'failed') : 'idle', problems }
  }),
)

function runTests() {
  actions.runTests.mutate(props.workflow.id, { onSuccess: (list) => (results.value = list) })
}
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0" data-tour="workflow-tests">
    <ul v-if="props.workflow.tests.length" class="grid gap-1">
      <li v-for="{ test, state, problems } in rows" :key="test.id" class="flex items-start gap-2 text-sm">
        <CircleCheck v-if="state === 'passed'" class="mt-0.5 size-4 shrink-0 text-primary" />
        <XCircle v-else-if="state === 'failed'" class="mt-0.5 size-4 shrink-0 text-destructive" />
        <span v-else class="mt-0.5 size-4 shrink-0 rounded-full border border-border" />
        <span class="min-w-0 flex-1">
          <span class="block">{{ test.name }}</span>
          <span v-if="state === 'failed'" class="block text-xs text-destructive">{{ problems }}</span>
        </span>
        <ConfirmButton
          :label="t('workflow.tests.delete')"
          :confirm-label="t('actions.confirmDeleteNamed', { name: test.name })"
          @confirm="actions.deleteTest.mutate({ workflow_id: props.workflow.id, test_id: test.id })"
        >
          <Trash2 class="size-4" />
        </ConfirmButton>
      </li>
    </ul>
    <p class="text-xs text-muted-foreground">
      {{ props.workflow.tests.length ? t('workflow.tests.hint') : t('workflow.tests.empty') }}
    </p>
    <p v-if="props.dirty && props.workflow.tests.length" class="text-xs text-muted-foreground">
      {{ t('workflow.tests.savedOnly') }}
    </p>
    <Button
      v-if="props.workflow.tests.length"
      size="sm"
      variant="outline"
      class="justify-self-start"
      :disabled="actions.runTests.isPending.value"
      @click="runTests"
    >
      <FlaskConical class="size-4" />
      {{ t('workflow.tests.run') }}
    </Button>
  </div>
</template>
