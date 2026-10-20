import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/**
 * The workflow behind an id, and whether it is an automation built from its
 * form (shown in the editor, not saved there). Shares the editor's query.
 */
export function useAutomationWorkflow(workflowId: () => string | null) {
  const workflow = useQuery({
    // no placeholder: another workflow; the banner must not show the one before
    queryKey: computed(() => WORKFLOW_KEYS.one(workflowId() ?? '')),
    queryFn: () => call<Workflow>('get_workflow', { id: workflowId() }),
    enabled: computed(() => Boolean(workflowId())),
  })
  const isAutomation = computed(() => Boolean(workflowId() && workflow.data.value?.automation))
  return { workflow, isAutomation }
}
