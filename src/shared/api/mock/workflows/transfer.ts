/** Workflow files: what an export holds and how an import swaps wallets. */

import { mockStores } from '../demo'
import { emptyGraph } from '@/shared/lib/workflow'
import { find, save } from './state'

const FORMAT = 'trade-app-workflow'

type FileStore = { id: string; name: string }

/** Every string in the value, nested ones included, in document order. */
function strings(value: unknown, out: string[] = []): string[] {
  if (typeof value === 'string') out.push(value)
  else if (Array.isArray(value)) for (const item of value) strings(item, out)
  else if (value && typeof value === 'object') for (const item of Object.values(value)) strings(item, out)
  return out
}

/** Swap every string equal to a key of `map` for its value. */
function swap(value: unknown, map: Record<string, string>): unknown {
  if (typeof value === 'string') return map[value] ?? value
  if (Array.isArray(value)) return value.map((item) => swap(item, map))
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, swap(item, map)]))
  }
  return value
}

function parse(json: unknown): Record<string, unknown> {
  let file: Record<string, unknown>
  try {
    file = JSON.parse(String(json ?? '{}')) as Record<string, unknown>
  } catch (err) {
    throw { code: 'INVALID_INPUT', message: `not a workflow file: ${String(err)}` }
  }
  if (file.format !== FORMAT) throw { code: 'INVALID_INPUT', message: 'not a workflow file' }
  return file
}

export function exportWorkflow(id: string): string {
  const workflow = find(id)
  const graph = JSON.parse(workflow.graph || JSON.stringify(emptyGraph()))
  const stores: FileStore[] = []
  for (const text of new Set(strings(graph))) {
    const store = mockStores.find((entry) => entry.id === text)
    if (store) stores.push({ id: store.id, name: store.name })
  }
  const file = { format: FORMAT, version: 1, name: workflow.name, description: workflow.description, graph, stores }
  return JSON.stringify(file, null, 2)
}

export function previewImport(json: unknown) {
  const file = parse(json)
  const nodes = ((file.graph as { nodes?: { kind?: string }[] } | undefined)?.nodes ?? []).filter(
    (node) => !String(node.kind ?? '').startsWith('trigger.'),
  )
  const missing = ((file.stores as FileStore[] | undefined) ?? []).filter(
    (store) => !mockStores.some((entry) => entry.id === store.id),
  )
  return { name: String(file.name ?? ''), steps: nodes.length, missing }
}

export function importWorkflow(input: Record<string, unknown> | undefined) {
  const file = parse(input?.json)
  const map = (input?.stores as Record<string, string> | undefined) ?? {}
  for (const id of Object.values(map)) {
    if (!mockStores.some((entry) => entry.id === id)) throw { code: 'INVALID_INPUT', message: `unknown wallet ${id}` }
  }
  return save(null, {
    name: String(file.name ?? ''),
    description: String(file.description ?? ''),
    active: false,
    graph: JSON.stringify(swap(file.graph ?? emptyGraph(), map)),
    react_to_workflows: false,
    max_amount: null,
    max_amount_currency: null,
  })
}
