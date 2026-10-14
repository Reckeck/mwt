import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { notify } from '@/shared/lib/notify'
import { WORKFLOW_KEYS, outcomeDetail, outcomeKey } from '@/shared/lib/workflow'

/**
 * What the user does with a workflow: save, switch on or off, run, emulate,
 * delete, move as a file; and with what it left: the queue and variables.
 * Every action says how it went.
 */
export function useWorkflowActions() {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  function refresh(ledger = false) {
    for (const queryKey of [
      WORKFLOW_KEYS.list,
      WORKFLOW_KEYS.runs,
      WORKFLOW_KEYS.queue,
      WORKFLOW_KEYS.waits,
      WORKFLOW_KEYS.vars,
    ]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    if (ledger) for (const queryKey of LEDGER_KEYS) void queryClient.invalidateQueries({ queryKey })
  }
  const failed = (title: string) => (err: unknown) => notify.error(err, title)

  /** A run's outcome as a toast. */
  function announce(report: RunReport, dry: boolean) {
    const key = outcomeKey(report.run.outcome)
    const detail = outcomeDetail(report.run.outcome)
    const made = report.transactions.length
    const description = [
      made ? t(dry ? 'workflow.run.wouldMake' : 'workflow.run.made', { count: made }) : '',
      report.run.queued ? t('workflow.run.queued', { count: report.run.queued }) : '',
      detail,
    ]
      .filter(Boolean)
      .join(' · ')
    const title = t(`workflow.run.${dry ? 'dry' : 'live'}.${key}`)
    if (key === 'failed') notify.error(null, title, { description })
    else notify.success(title, { description: description || undefined })
  }

  const save = useMutation({
    mutationFn: (args: { id: string | null; draft: WorkflowDraft }) =>
      call<SavedWorkflow>('save_workflow', { input: { id: args.id, workflow: args.draft } }),
    onSuccess: (saved) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(saved.workflow.id), saved.workflow)
      refresh()
    },
    meta: { success: 'workflow.saved', error: 'workflow.saveFailed' },
  })

  const setActive = useMutation({
    mutationFn: (args: { id: string; active: boolean }) => call<Workflow>('set_workflow_active', { input: args }),
    onSuccess: (workflow) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
      refresh()
    },
    meta: {
      error: 'workflow.switchFailed',
      success: (data) => t((data as Workflow).active ? 'workflow.switchedOn' : 'workflow.switchedOff'),
    },
  })

  const remove = useMutation({
    mutationFn: (id: string) => call<void>('delete_workflow', { id }),
    onSuccess: () => {
      refresh()
    },
    meta: { success: 'workflow.deleted', error: 'workflow.deleteFailed' },
  })

  const dryRun = useMutation({
    mutationFn: (args: {
      graph: string
      workflowId: string | null
      triggerNode?: string | null
      answers?: Record<string, boolean>
    }) =>
      call<RunReport>('dry_run_workflow', {
        input: {
          graph: args.graph,
          workflow_id: args.workflowId,
          trigger_node: args.triggerNode ?? null,
          answers: args.answers ?? {},
        },
      }),
    onSuccess: (report) => announce(report, true),
    onError: failed('workflow.run.failed'),
    meta: { silent: 'announce() reports how the run went, failures included' },
  })

  const runNow = useMutation({
    mutationFn: (args: { id: string; triggerNode?: string | null }) =>
      call<RunReport>('run_workflow_now', { input: { id: args.id, trigger_node: args.triggerNode ?? null } }),
    onSuccess: (report) => {
      announce(report, false)
      refresh(true)
    },
    onError: failed('workflow.run.failed'),
    meta: { silent: 'announce() reports how the run went, failures included' },
  })

  const exportFile = useMutation({
    mutationFn: async (id: string) => {
      const text = await call<string>('export_workflow', { id })
      await navigator.clipboard.writeText(text)
      return text
    },
    meta: { success: 'workflow.exported', error: 'workflow.exportFailed' },
  })

  const importFile = useMutation({
    mutationFn: (json: string) => call<SavedWorkflow>('import_workflow', { json }),
    onSuccess: () => {
      refresh()
    },
    meta: { success: 'workflow.imported', error: 'workflow.importFailed' },
  })

  const confirmQueued = useMutation({
    mutationFn: (id: string) => call<Transaction[]>('confirm_workflow_queued', { id }),
    onSuccess: () => refresh(true),
    meta: {
      error: 'workflow.queue.confirmFailed',
      success: (data) => t('workflow.queue.confirmed', { count: (data as Transaction[]).length }),
    },
  })

  const discardQueued = useMutation({
    mutationFn: (id: string) => call<void>('discard_workflow_queued', { id }),
    onSuccess: () => {
      refresh()
    },
    meta: { success: 'workflow.queue.discarded', error: 'workflow.queue.discardFailed' },
  })

  const saveDraft = useMutation({
    mutationFn: (input: { id: string; graph: string }) => call<Workflow>('save_workflow_draft', { input }),
    onSuccess: (workflow) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
    },
    meta: { success: 'workflow.draft.saved', error: 'workflow.draft.saveFailed' },
  })

  const discardDraft = useMutation({
    mutationFn: (id: string) => call<Workflow>('discard_workflow_draft', { id }),
    onSuccess: (workflow) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
    },
    meta: { success: 'workflow.draft.dropped', error: 'workflow.draft.dropFailed' },
  })

  const answerWait = useMutation({
    mutationFn: (input: { id: string; yes: boolean }) => call<RunReport>('answer_workflow_wait', { input }),
    onSuccess: (report) => {
      announce(report, false)
      refresh(true)
    },
    onError: failed('workflow.queue.answerFailed'),
    meta: { silent: 'announce() reports how the run went, failures included' },
  })

  const setVar = useMutation({
    mutationFn: (input: { scope: VarScope; name: string; value: string }) => call<void>('set_workflow_var', { input }),
    onSuccess: () => {
      refresh()
    },
    meta: { success: 'workflow.vars.saved', error: 'workflow.vars.saveFailed' },
  })

  const deleteVar = useMutation({
    mutationFn: (input: { scope: VarScope; name: string }) => call<void>('delete_workflow_var', { input }),
    onSuccess: () => {
      refresh()
    },
    meta: { success: 'workflow.vars.deleted', error: 'workflow.vars.deleteFailed' },
  })

  const restoreVersion = useMutation({
    mutationFn: (input: { id: string; saved_at: string }) => call<SavedWorkflow>('restore_workflow_version', { input }),
    onSuccess: (saved) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(saved.workflow.id), saved.workflow)
      refresh()
    },
    meta: { success: 'workflow.versions.restored', error: 'workflow.versions.restoreFailed' },
  })

  const saveTest = useMutation({
    mutationFn: (input: { workflow_id: string; test: WorkflowTest }) => call<Workflow>('save_workflow_test', { input }),
    onSuccess: (workflow) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
    },
    meta: { success: 'workflow.tests.saved', error: 'workflow.tests.saveFailed' },
  })

  const deleteTest = useMutation({
    mutationFn: (input: { workflow_id: string; test_id: string }) => call<Workflow>('delete_workflow_test', { input }),
    onSuccess: (workflow) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
    },
    meta: { success: 'workflow.tests.deleted', error: 'workflow.tests.deleteFailed' },
  })

  const runTests = useMutation({
    mutationFn: (id: string) => call<WorkflowTestResult[]>('run_workflow_tests', { id }),
    onSuccess: (results) => {
      const failedCount = results.filter((result) => !result.passed).length
      if (failedCount)
        notify.error(null, t('workflow.tests.someFailed', { count: failedCount, total: results.length }), {
          description: '',
        })
      else notify.success(t('workflow.tests.allPassed', { total: results.length }))
    },
    onError: failed('workflow.tests.runFailed'),
    meta: { silent: 'the toast in onSuccess says how many tests passed' },
  })

  return {
    restoreVersion,
    saveTest,
    deleteTest,
    runTests,
    save,
    setActive,
    remove,
    dryRun,
    runNow,
    exportFile,
    importFile,
    confirmQueued,
    discardQueued,
    answerWait,
    saveDraft,
    discardDraft,
    setVar,
    deleteVar,
  }
}
