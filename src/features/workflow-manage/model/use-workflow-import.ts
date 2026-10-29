import { computed, ref, watch, type Ref } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'

import { useWorkflowActions } from './use-workflow-actions'

/**
 * State of `Import an automation` (Auto-3 · 17): the chosen file's text, what
 * the core finds in it, and the wallet picked instead of each missing one.
 */
export function useWorkflowImport(open: Ref<boolean>, onImported: (workflow: Workflow) => void) {
  const { t } = useI18n()
  const { importFile } = useWorkflowActions()

  const fileName = ref('')
  const text = ref('')
  const dragging = ref(false)
  /** Missing wallet id from the file → the id picked here. */
  const stores = ref<Record<string, string>>({})

  const preview = useQuery({
    queryKey: computed(() => ['workflow-import-preview', text.value]),
    queryFn: () => call<WorkflowImportPreview>('preview_workflow_import', { json: text.value }),
    enabled: computed(() => text.value !== ''),
    placeholderData: keepPreviousData,
    retry: false,
  })
  const found = computed(() => (text.value && !preview.isError.value ? (preview.data.value ?? null) : null))
  const missing = computed(() => found.value?.missing ?? [])

  watch(open, (value) => {
    if (!value) return
    fileName.value = ''
    text.value = ''
    stores.value = {}
  })
  watch(
    () => preview.data.value?.name,
    () => {
      stores.value = {}
    },
  )

  async function pick(file: File | undefined) {
    if (!file) return
    fileName.value = file.name
    text.value = await file.text()
  }

  const form = useFormValidation<string>(
    () => ({
      file:
        text.value && found.value
          ? null
          : t(preview.isError.value ? 'workflow.import.notWorkflow' : 'workflow.import.noFile'),
      ...Object.fromEntries(
        missing.value.map((entry) => [`store-${entry.id}`, required(stores.value[entry.id], 'pick')]),
      ),
    }),
    { resetOn: open },
  )

  function submit() {
    importFile.mutate(
      { json: text.value, stores: stores.value },
      {
        onSuccess: (saved) => {
          open.value = false
          onImported(saved.workflow)
        },
      },
    )
  }

  return { fileName, dragging, stores, found, missing, form, pick, submit, pending: importFile.isPending }
}
