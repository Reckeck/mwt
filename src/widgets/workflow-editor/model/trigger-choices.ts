import { localized } from '@/shared/lib/workflow'

import { nodeSummary } from './fields'
import type { EditorNode } from './flow'

/** A trigger the run buttons can start from. */
export interface TriggerChoice {
  id: string
  title: string
  /** What it watches, in words: «every day, 09:00». */
  detail: string
}

/** The workflow's triggers that are on, «Запуск вручную» first: it is the default. */
export function triggerChoices(
  nodes: EditorNode[],
  def: (kind: string) => NodeDef | undefined,
  locale: string,
): TriggerChoice[] {
  return nodes
    .flatMap((node) => {
      const nodeDef = def(node.data?.kind ?? '')
      if (nodeDef?.group !== 'trigger' || node.data?.disabled) return []
      return [
        {
          id: node.id,
          title: node.data?.label || localized(nodeDef.title, locale),
          detail: nodeSummary(nodeDef, node.data?.params ?? {}, locale),
          manual: nodeDef.kind === 'trigger.manual',
        },
      ]
    })
    .sort((left, right) => Number(right.manual) - Number(left.manual))
    .map(({ manual: _manual, ...choice }) => choice)
}
