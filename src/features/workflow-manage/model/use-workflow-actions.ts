import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'
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
    for (const queryKey of [WORKFLOW_KEYS.list, WORKFLOW_KEYS.runs, WORKFLOW_KEYS.queue, WORKFLOW_KEYS.vars]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    if (ledger) for (const queryKey of LEDGER_KEYS) void queryClient.invalidateQueries({ queryKey })
  }
  const failed = (title: string) => (err: unknown) =>
    toast.error(t(title), { description: codeErrorMessage(parseError(err), t) })

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
    if (key === 'failed') toast.error(title, { description })
    else toast.success(title, { description: description || undefined })
  }

  const save = useMutation({
    mutationFn: (args: { id: string | null; draft: WorkflowDraft }) =>
      call<SavedWorkflow>('save_workflow', { input: { id: args.id, workflow: args.draft } }),
    onSuccess: (saved) => {
      void queryClient.setQueryData(WORKFLOW_KEYS.one(saved.workflow.id), saved.workflow)
      toast.success(t('workflow.saved'))
      refresh()
    },
    onError: failed('workflow.saveFailed'),
  })

  const setActive = useMutation({
    mutationFn: (args: { id: string; active: boolean }) => call<Workflow>('set_workflow_active', { input: args }),
    onSuccess: (workflow) => {
      toast.success(t(workflow.active ? 'workflow.switchedOn' : 'workflow.switchedOff'))
      void queryClient.setQueryData(WORKFLOW_KEYS.one(workflow.id), workflow)
      refresh()
    },
    onError: failed('workflow.switchFailed'),
  })

  const remove = useMutation({
    mutationFn: (id: string) => call<void>('delete_workflow', { id }),
    onSuccess: () => {
      toast.success(t('workflow.deleted'))
      refresh()
    },
    onError: failed('workflow.deleteFailed'),
  })

  const dryRun = useMutation({
    mutationFn: (args: { graph: string; workflowId: string | null; triggerNode?: string | null }) =>
      call<RunReport>('dry_run_workflow', {
        input: { graph: args.graph, workflow_id: args.workflowId, trigger_node: args.triggerNode ?? null },
      }),
    onSuccess: (report) => announce(report, true),
    onError: failed('workflow.run.failed'),
  })

  const runNow = useMutation({
    mutationFn: (args: { id: string; triggerNode?: string | null }) =>
      call<RunReport>('run_workflow_now', { input: { id: args.id, trigger_node: args.triggerNode ?? null } }),
    onSuccess: (report) => {
      announce(report, false)
      refresh(true)
    },
    onError: failed('workflow.run.failed'),
  })

  const exportFile = useMutation({
    mutationFn: async (id: string) => {
      const text = await call<string>('export_workflow', { id })
      await navigator.clipboard.writeText(text)
      return text
    },
    onSuccess: () => toast.success(t('workflow.exported')),
    onError: failed('workflow.exportFailed'),
  })

  const importFile = useMutation({
    mutationFn: (json: string) => call<SavedWorkflow>('import_workflow', { json }),
    onSuccess: () => {
      toast.success(t('workflow.imported'))
      refresh()
    },
    onError: failed('workflow.importFailed'),
  })

  const confirmQueued = useMutation({
    mutationFn: (id: string) => call<Transaction[]>('confirm_workflow_queued', { id }),
    onSuccess: (made) => {
      toast.success(t('workflow.queue.confirmed', { count: made.length }))
      refresh(true)
    },
    onError: failed('workflow.queue.confirmFailed'),
  })

  const discardQueued = useMutation({
    mutationFn: (id: string) => call<void>('discard_workflow_queued', { id }),
    onSuccess: () => {
      toast.success(t('workflow.queue.discarded'))
      refresh()
    },
    onError: failed('workflow.queue.discardFailed'),
  })

  const setVar = useMutation({
    mutationFn: (input: { scope: VarScope; name: string; value: string }) => call<void>('set_workflow_var', { input }),
    onSuccess: () => {
      toast.success(t('workflow.vars.saved'))
      refresh()
    },
    onError: failed('workflow.vars.saveFailed'),
  })

  const deleteVar = useMutation({
    mutationFn: (input: { scope: VarScope; name: string }) => call<void>('delete_workflow_var', { input }),
    onSuccess: () => {
      toast.success(t('workflow.vars.deleted'))
      refresh()
    },
    onError: failed('workflow.vars.deleteFailed'),
  })

  return {
    save,
    setActive,
    remove,
    dryRun,
    runNow,
    exportFile,
    importFile,
    confirmQueued,
    discardQueued,
    setVar,
    deleteVar,
  }
}
