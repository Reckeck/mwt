import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** The ready-made templates and «Создать» from one of them. */
export function useTemplateActions() {
  const queryClient = useQueryClient()
  const templates = useQuery({
    queryKey: ['workflow-templates'],
    queryFn: () => call<WorkflowTemplateInfo[]>('workflow_templates'),
  })
  const create = useMutation({
    mutationFn: (input: WorkflowFromTemplateInput) => call<SavedWorkflow>('workflow_from_template', { input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: [...WORKFLOW_KEYS.list] })
      void queryClient.invalidateQueries({ queryKey: [...WORKFLOW_KEYS.vars] })
    },
    meta: { success: 'workflowTemplates.created', error: 'workflowTemplates.createFailed' },
  })
  return { templates, create }
}
