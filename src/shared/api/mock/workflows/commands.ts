/** The workflow commands. */

import { emptyGraph, parseGraph } from '@/shared/lib/workflow'
import { fakeRun, keepQuestion, validate } from './engine'
import { catalog, find, now, queue, runs, sameScope, save, uuid, vars, waits, workflows } from './state'
import { fromTemplate, templates } from './templates'

/** Answer a workflow command, or `undefined` when `cmd` is not one. */
export function mockWorkflows(cmd: string, args: Record<string, unknown> | undefined): unknown {
  const input = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'workflow_node_catalog':
      return catalog
    case 'validate_workflow':
      return validate(parseGraph(String(input?.graph ?? '')))
    case 'list_workflows':
      return workflows
    case 'get_workflow':
      return find(String(args?.id))
    case 'save_workflow':
      return save((input?.id as string | null) || null, input?.workflow as WorkflowDraft)
    case 'set_workflow_active': {
      const workflow = find(String(input?.id))
      if (input?.active && validate(parseGraph(workflow.graph)).length) {
        throw { code: 'INVALID_INPUT', message: "fix the workflow's errors before switching it on" }
      }
      workflow.active = Boolean(input?.active)
      return workflow
    }
    case 'delete_workflow': {
      const index = workflows.findIndex((entry) => entry.id === args?.id)
      if (index >= 0) workflows.splice(index, 1)
      return null
    }
    case 'restore_workflow_version': {
      const workflow = find(String(input?.id))
      const version = workflow.versions.find((entry) => entry.saved_at === input?.saved_at)
      if (!version) throw { code: 'MISSING', message: 'version' }
      return save(workflow.id, { ...workflow, graph: version.graph, active: false })
    }
    case 'dry_run_workflow':
      return fakeRun(
        parseGraph(String(input?.graph ?? '')),
        String(input?.workflow_id ?? 'draft'),
        (input?.trigger_node as string) ?? null,
        true,
        (input?.answers as Record<string, boolean> | undefined) ?? {},
      )
    case 'run_workflow_now': {
      const workflow = find(String(input?.id))
      const report = fakeRun(parseGraph(workflow.graph), workflow.id, (input?.trigger_node as string) ?? null, false)
      runs.unshift(report.run)
      keepQuestion(report)
      return report
    }
    case 'save_workflow_draft': {
      const workflow = find(String(input?.id))
      workflow.draft = { saved_at: now(), graph: String(input?.graph ?? '') }
      return workflow
    }
    case 'discard_workflow_draft': {
      const workflow = find(String(args?.id))
      workflow.draft = undefined
      return workflow
    }
    case 'list_workflow_waits':
      return waits
    case 'answer_workflow_wait': {
      const index = waits.findIndex((entry) => entry.id === input?.id && entry.kind === 'confirm')
      if (index < 0) throw { code: 'MISSING', message: `question ${String(input?.id)}` }
      const [wait] = waits.splice(index, 1)
      const workflow = find(wait.workflow_id)
      const port = input?.yes ? 'true' : 'false'
      const report = fakeRun(parseGraph(workflow.graph), workflow.id, null, false, {}, { nodeId: wait.node_id, port })
      runs.unshift(report.run)
      keepQuestion(report)
      return report
    }
    case 'list_workflow_runs':
      return runs.filter((run) => !input?.workflow_id || run.workflow_id === input.workflow_id)
    case 'list_workflow_queue':
      return queue
    case 'confirm_workflow_queued':
    case 'discard_workflow_queued': {
      const index = queue.findIndex((entry) => entry.id === args?.id)
      if (index >= 0) queue.splice(index, 1)
      return cmd === 'confirm_workflow_queued' ? [] : null
    }
    case 'list_workflow_vars':
      return vars
    case 'set_workflow_var': {
      const scope = input?.scope as VarScope
      const name = String(input?.name ?? '').trim()
      const existing = vars.find((entry) => sameScope(entry.scope, scope) && entry.name === name)
      if (existing) Object.assign(existing, { value: String(input?.value), updated_at: now() })
      else vars.push({ scope, name, value: String(input?.value), updated_at: now() })
      return null
    }
    case 'delete_workflow_var': {
      const index = vars.findIndex(
        (entry) => sameScope(entry.scope, input?.scope as VarScope) && entry.name === input?.name,
      )
      if (index >= 0) vars.splice(index, 1)
      return null
    }
    case 'export_workflow': {
      const workflow = find(String(args?.id))
      return JSON.stringify(
        {
          format: 'trade-app-workflow',
          version: 1,
          name: workflow.name,
          description: workflow.description,
          graph: JSON.parse(workflow.graph || JSON.stringify(emptyGraph())),
        },
        null,
        2,
      )
    }
    case 'workflow_templates':
      return templates
    case 'workflow_from_template':
      return fromTemplate(input as unknown as WorkflowFromTemplateInput)
    case 'import_workflow': {
      const file = JSON.parse(String(args?.json ?? '{}')) as Record<string, unknown>
      if (file.format !== 'trade-app-workflow') throw { code: 'INVALID_INPUT', message: 'not a workflow file' }
      return save(null, {
        name: String(file.name ?? ''),
        description: String(file.description ?? ''),
        active: false,
        graph: JSON.stringify(file.graph ?? emptyGraph()),
        react_to_workflows: false,
        max_amount: null,
      })
    }
    case 'save_workflow_test': {
      const workflow = find(String(input?.workflow_id))
      const test = { ...(input?.test as WorkflowTest) }
      if (!test.name?.trim()) throw { code: 'INVALID_INPUT', message: 'name the test' }
      if (!test.id) test.id = uuid()
      workflow.tests = [...workflow.tests.filter((entry) => entry.id !== test.id), test]
      return workflow
    }
    case 'delete_workflow_test': {
      const workflow = find(String(input?.workflow_id))
      workflow.tests = workflow.tests.filter((entry) => entry.id !== input?.test_id)
      return workflow
    }
    case 'run_workflow_tests': {
      const workflow = find(String(args?.id))
      return workflow.tests.map((test): WorkflowTestResult => {
        const report = fakeRun(parseGraph(workflow.graph), workflow.id, test.trigger_node, true)
        const got = report.run.outcome === 'done' ? 'done' : 'failed'
        const passed = got === test.expect.outcome
        return {
          test_id: test.id,
          name: test.name,
          passed,
          problems_ru: passed ? [] : [`итог «${got}» вместо «${test.expect.outcome}»`],
          problems_en: passed ? [] : [`outcome «${got}» instead of «${test.expect.outcome}»`],
          report,
        }
      })
    }
    default:
      return undefined
  }
}
