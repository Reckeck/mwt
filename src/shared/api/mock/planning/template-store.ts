/** The template store of the mock (spec op-period T2, T3): its records and the core's validation messages. */

import { mockCategories, mockContacts } from '../ledger'
import { mockStores } from '../demo'
import { checkRows, checkTargets } from './distribution-rule'
import { automations, fail } from './state'

/** Every template, live and archived. */
export const mockTemplates: OperationTemplate[] = []

const sameName = (left: string, right: string) => left.trim().toLowerCase() === right.trim().toLowerCase()

export function findTemplate(id: string): OperationTemplate {
  return mockTemplates.find((template) => template.id === id) ?? fail(`template ${id}`, 'MISSING')
}

/** T2: the rules of one template; its own record is not a duplicate of itself. */
export function checkTemplate(template: OperationTemplate): void {
  if (!template.name.trim()) fail('name the template')
  const taken = mockTemplates.some(
    (known) => known.id !== template.id && !known.archived && !template.archived && sameName(known.name, template.name),
  )
  if (taken) fail(`a template named ${template.name.trim()} already exists`)
  for (const key of template.categories) {
    if (!mockCategories.some((category) => category.key === key)) fail(`category ${key}`, 'MISSING')
  }
  if (template.store_id && !mockStores.some((store) => store.id === template.store_id)) {
    fail(`store ${template.store_id}`, 'MISSING')
  }
  if (template.contact_id && !mockContacts.some((contact) => contact.id === template.contact_id)) {
    fail(`contact ${template.contact_id}`, 'MISSING')
  }
  const rule = template.distribution
  if (!rule) return
  if (template.kind !== 'income') fail('only an income template can carry a distribution')
  checkRows(rule.rows, true)
  checkTargets(rule.rows)
}

export function made(input: OperationTemplateInput, base: Pick<OperationTemplate, 'id' | 'archived' | 'created_at'>) {
  const template: OperationTemplate = { ...base, ...input, name: input.name.trim() }
  checkTemplate(template)
  return template
}

export function replaced(template: OperationTemplate): OperationTemplate {
  mockTemplates.splice(
    mockTemplates.findIndex((known) => known.id === template.id),
    1,
    template,
  )
  return template
}

/** T3: a template is deleted only while no automation refers to it. */
export function deleteTemplate(id: string): OperationTemplate {
  const template = findTemplate(id)
  const user = automations.find((automation) => automation.operation_template_id === id)
  if (user) fail(`template ${template.name} is used by the automation ${user.name}; archive it instead`)
  mockTemplates.splice(mockTemplates.indexOf(template), 1)
  return template
}
