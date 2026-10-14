/** Workflow mock state: the catalog, the lists and saving a workflow. */

import { catalogMap, parseGraph } from '@/shared/lib/workflow'
import catalogJson from '../workflow-catalog.json'
import { validate } from './engine'

export const catalog = catalogJson as unknown as NodeDef[]
export const defs = catalogMap(catalog)

export const workflows: Workflow[] = []
export const runs: WorkflowRun[] = []
export const queue: QueuedActions[] = []
export const vars: WorkflowVar[] = []
export const waits: WorkflowWait[] = []

export const uuid = () => crypto.randomUUID()
export const now = () => new Date().toISOString().replace(/\.\d{3}Z$/, 'Z')

export function issue(
  code: string,
  ru: string,
  en: string,
  nodeId: string | null = null,
  edgeId: string | null = null,
) {
  return { level: 'error', code, node_id: nodeId, edge_id: edgeId, text_ru: ru, text_en: en } as GraphIssue
}
export function find(id: string): Workflow {
  const workflow = workflows.find((entry) => entry.id === id)
  if (!workflow) throw { code: 'MISSING', message: `workflow ${id}` }
  return workflow
}

export function save(id: string | null, draft: WorkflowDraft): SavedWorkflow {
  if (!draft.name.trim()) throw { code: 'INVALID_INPUT', message: 'name the workflow' }
  const issues = validate(parseGraph(draft.graph))
  if (draft.active && issues.some((entry) => entry.level === 'error')) {
    throw { code: 'INVALID_INPUT', message: `fix the workflow's errors before switching it on: ${issues[0].text_en}` }
  }
  const stamp = now()
  if (!id) {
    const workflow: Workflow = {
      id: uuid(),
      name: draft.name.trim(),
      description: draft.description,
      active: draft.active,
      graph: draft.graph,
      created_at: stamp,
      updated_at: stamp,
      react_to_workflows: draft.react_to_workflows,
      max_amount: draft.max_amount,
      versions: [],
      tests: [],
    }
    workflows.push(workflow)
    return { workflow, issues }
  }
  const workflow = find(id)
  if (workflow.graph !== draft.graph) {
    workflow.versions = [...workflow.versions, { saved_at: workflow.updated_at, graph: workflow.graph }].slice(-20)
  }
  Object.assign(workflow, { ...draft, name: draft.name.trim(), updated_at: stamp, draft: undefined })
  return { workflow, issues }
}

export const sameScope = (left: VarScope, right: VarScope) => JSON.stringify(left) === JSON.stringify(right)
