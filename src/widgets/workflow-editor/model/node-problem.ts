import { localized } from '@/shared/lib/workflow'

type Translate = (key: string, values?: Record<string, unknown>) => string

/**
 * What stops a node, in a few words over the card (Auto-3 · 16): `fill in
 * `To`» for an empty required field, the check's own text for anything else.
 * Warnings do not count: the scheme can be switched on with them.
 */
export function nodeProblem(issues: GraphIssue[], def: NodeDef | undefined, locale: string, t: Translate): string {
  const error = issues.find((issue) => issue.level === 'error')
  if (!error) return ''
  const param = error.param ? def?.params.find((entry) => entry.name === error.param) : undefined
  if (param) return t('workflow.node.fillIn', { field: localized(param.label, locale) })
  return locale.startsWith('ru') ? error.text_ru : error.text_en
}

/** `Cannot turn on: 1 error` in place of `Off` while errors keep the scheme off. */
export function activeStatus(active: boolean, blockers: number, t: Translate, form: string): string {
  if (!active && blockers) return t(`workflow.cantActivate.${form}`, { count: blockers })
  return t(active ? 'workflow.active' : 'workflow.inactive')
}
