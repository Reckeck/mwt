import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import type { useWorkflowActions } from '@/features/workflow-manage'
import { outcomeKey } from '@/shared/lib/workflow'

import type { EditorGraph } from './use-editor-graph'
import { dateFormat } from '@/shared/lib/format'

/**
 * The last emulation or real run: shown on the canvas (a step on every node,
 * a value on every wire), answered step by step, kept as a test.
 */
export function useRunReport(
  editor: EditorGraph,
  actions: ReturnType<typeof useWorkflowActions>,
  workflowId: () => string | null,
) {
  const { t, locale } = useI18n()
  const report = ref<RunReport | null>(null)
  /** Answers given to «Ждать подтверждения» in the current emulation, by node id. */
  const answers = ref<Record<string, boolean>>({})

  const stepOf = computed(() => {
    const map = new Map<string, TraceStep>()
    for (const step of report.value?.run.trace ?? []) map.set(step.node_id, step)
    return map
  })
  watch(report, (value) => {
    const labels = value?.edge_values ?? {}
    editor.edges.value = editor.edges.value.map((edge) => ({ ...edge, label: labels[edge.id] ?? undefined }))
  })

  /** The trigger the last emulation started from: an answer goes on from it. */
  const fromTrigger = ref<string | null>(null)

  function emulate(keepAnswers = false, triggerNode?: string | null) {
    if (keepAnswers !== true) answers.value = {}
    if (triggerNode !== undefined) fromTrigger.value = triggerNode
    actions.dryRun.mutate(
      {
        graph: editor.graphJson.value,
        workflowId: workflowId(),
        triggerNode: fromTrigger.value,
        answers: answers.value,
      },
      { onSuccess: (result) => (report.value = result) },
    )
  }

  /** The emulation asked: remember the answer and run it again along that way. */
  function answer(nodeId: string, yes: boolean) {
    answers.value = { ...answers.value, [nodeId]: yes }
    emulate(true)
  }

  function runNow(triggerNode: string | null = null) {
    const id = workflowId()
    if (!id) return
    actions.runNow.mutate({ id, triggerNode }, { onSuccess: (result) => (report.value = result) })
  }

  /** Keep what the last emulation did as a test of this workflow. */
  function saveAsTest() {
    const result = report.value
    const id = workflowId()
    if (!id || !result) return
    const queued = result.queued.reduce((sum, batch) => sum + batch.transactions.length, 0)
    actions.saveTest.mutate({
      workflow_id: id,
      test: {
        id: '',
        name: t('workflow.tests.defaultName', {
          when: dateFormat({ dateStyle: 'short', timeStyle: 'short', hourCycle: 'h23' }, locale.value).format(
            new Date(),
          ),
        }),
        trigger_node: result.run.trace[0]?.node_id ?? null,
        at: result.run.started_at,
        answers: { ...answers.value },
        expect: {
          // A question left unanswered is a run that waits.
          outcome: outcomeKey(result.run.outcome) === 'asking' ? 'waiting' : outcomeKey(result.run.outcome),
          transactions: result.transactions.length,
          queued,
          notices: result.notices.length,
        },
      },
    })
  }

  return { report, answers, stepOf, emulate, answer, runNow, saveAsTest }
}
