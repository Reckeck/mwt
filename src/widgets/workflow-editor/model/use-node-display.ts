import type { ComputedRef } from 'vue'
import { useI18n } from 'vue-i18n'

import type { WorkflowNodeData } from '@/entities/workflow'

import type { EditorNode } from './flow'
import { nodeProblem } from './node-problem'
import type { EditorGraph } from './use-editor-graph'
import type { useSchemeWords } from './use-scheme-words'

/** What a node card shows: its definition, issues, value in words, wired ports and the emulated step. */
export function useNodeDisplay(
  editor: EditorGraph,
  issuesOf: (id: string) => GraphIssue[],
  scheme: ReturnType<typeof useSchemeWords>,
  run: { stepOf: ComputedRef<Map<string, TraceStep>>; orderOf: ComputedRef<Map<string, number>> },
) {
  const { t, locale } = useI18n()
  return (id: string, data: EditorNode['data']): WorkflowNodeData => ({
    def: editor.def(data?.kind ?? ''),
    kind: data?.kind ?? '',
    label: data?.label ?? '',
    disabled: data?.disabled ?? false,
    issues: issuesOf(id),
    problem: nodeProblem(issuesOf(id), editor.def(data?.kind ?? ''), locale.value, t),
    summary: scheme.summaryOf(id, data?.kind ?? '', data?.params ?? {}),
    linked: [...(scheme.linked.value.get(id) ?? [])],
    step: run.stepOf.value.get(id) ?? null,
    order: run.orderOf.value.get(id) ?? null,
  })
}
