import { type Ref, computed, onScopeDispose, ref, watch } from 'vue'

import { call } from '@/shared/api/ipc'

/** The backend's checks of the graph, rerun 300 ms after each edit. */
export function useGraphValidation(graphJson: Ref<string>) {
  const issues = ref<GraphIssue[]>([])
  let timer: ReturnType<typeof setTimeout> | undefined
  watch(
    graphJson,
    (json) => {
      clearTimeout(timer)
      timer = setTimeout(async () => {
        try {
          issues.value = await call<GraphIssue[]>('validate_workflow', { input: { graph: json } })
        } catch {
          // A graph that does not parse is reported by «Сохранить».
        }
      }, 300)
    },
    { immediate: true },
  )
  onScopeDispose(() => clearTimeout(timer))

  const issuesOf = (nodeId: string) => issues.value.filter((issue) => issue.node_id === nodeId)
  const errors = computed(() => issues.value.filter((issue) => issue.level === 'error'))
  return { issues, issuesOf, errors }
}
