import { type Ref, computed, nextTick, onScopeDispose, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { useEventListener } from '@vueuse/core'

import { WORKFLOW_LESSONS, WORKFLOW_TEMPLATES, type useNodeCatalog } from '@/entities/workflow'
import type { useWorkflowActions } from '@/features/workflow-manage'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { WORKFLOW_KEYS, emptyGraph, parseGraph } from '@/shared/lib/workflow'

import { useHistory } from './history'
import type { EditorGraph } from './use-editor-graph'

type Settings = [name: string, description: string, active: boolean, react: boolean, maxAmount: string]

interface DocumentSource {
  workflowId: () => string | null
  template: () => string | null
  lesson: () => string | null
}

/**
 * The workflow being edited as a document: where it starts from (stored,
 * lesson, template, empty), its settings, unsaved state, undo history, saving,
 * a draft kept aside, a version put back.
 */
export function useWorkflowDocument(
  editor: EditorGraph,
  catalogQuery: ReturnType<typeof useNodeCatalog>['query'],
  actions: ReturnType<typeof useWorkflowActions>,
  report: Ref<RunReport | null>,
  source: DocumentSource,
) {
  const { t } = useI18n()
  const { graphJson, loadGraph } = editor

  const name = ref('')
  const description = ref('')
  const active = ref(false)
  const react = ref(false)
  const maxAmount = ref('')
  /** Currency the stored cap is in; empty for a new or an old cap, which the backend labels on save. */
  const capCurrency = ref('')
  const loaded = ref(false)
  const savedSnapshot = ref('')
  /** What `Undo the edits` goes back to: the saved workflow, or the start of a new one. */
  const savedState = ref<{ graph: string; settings: Settings } | null>(null)

  const settingsOf = (): Settings => [name.value, description.value, active.value, react.value, maxAmount.value]
  const settingsJson = computed(() => JSON.stringify(settingsOf()))
  const snapshot = () => graphJson.value + settingsJson.value
  const dirty = computed(() => loaded.value && savedSnapshot.value !== snapshot())

  const lesson = computed(() => WORKFLOW_LESSONS.find((entry) => entry.id === source.lesson()) ?? null)
  const workflow = useQuery({
    // no placeholder: another workflow; the editor fills its document from it
    queryKey: computed(() => WORKFLOW_KEYS.one(source.workflowId() ?? '')),
    queryFn: () => call<Workflow>('get_workflow', { id: source.workflowId() }),
    enabled: computed(() => Boolean(source.workflowId())),
  })

  /* --- undo / redo --- */
  let restoring = false
  const history = useHistory((json) => {
    restoring = true
    loadGraph(parseGraph(json))
    void nextTick(() => {
      restoring = false
    })
  })
  let recordTimer: ReturnType<typeof setTimeout> | undefined
  watch(graphJson, (json) => {
    if (!loaded.value || restoring) return
    clearTimeout(recordTimer)
    recordTimer = setTimeout(() => history.record(json), 250)
  })
  onScopeDispose(() => clearTimeout(recordTimer))

  function rememberSaved() {
    savedState.value = { graph: graphJson.value, settings: settingsOf() }
  }

  /* Loads after everything it touches is declared: a cached workflow fires at once. */
  watch(
    [() => workflow.data.value, () => catalogQuery.data.value, source.template],
    ([stored, catalog]) => {
      if (loaded.value || !catalog) return
      if (source.workflowId()) {
        if (!stored) return
        name.value = stored.name
        description.value = stored.description
        active.value = stored.active
        react.value = stored.react_to_workflows
        maxAmount.value = stored.max_amount ?? ''
        capCurrency.value = stored.max_amount_currency ?? ''
        loadGraph(parseGraph(stored.graph))
      } else if (lesson.value) {
        name.value = t('workflow.lesson.workflowName', { title: t(`workflow.lesson.${lesson.value.id}.title`) })
        loadGraph(lesson.value.start((key) => t(key)))
      } else {
        const template = WORKFLOW_TEMPLATES.find((entry) => entry.id === source.template())
        name.value = template ? t(`workflow.template.${template.id}.name`) : t('workflow.newName')
        loadGraph(template ? template.graph((key) => t(key)) : emptyGraph())
      }
      loaded.value = true
      // A new workflow nobody touched is not «unsaved edits»: leaving it asks nothing.
      // Taken after the canvas settles, since mounting nodes measures them into the graph.
      savedSnapshot.value = snapshot()
      if (!source.workflowId()) {
        void nextTick(() => {
          savedSnapshot.value = snapshot()
        })
      }
      rememberSaved()
      history.reset(graphJson.value)
      // Not below half size: a long graph is panned rather than shrunk past reading.
      void nextTick(() => editor.flow.fitView({ padding: 0.2, maxZoom: 1, minZoom: 0.5 }))
    },
    { immediate: true },
  )

  const draft = computed<WorkflowDraft>(() => ({
    name: name.value.trim(),
    description: description.value,
    active: active.value,
    graph: graphJson.value,
    react_to_workflows: react.value,
    max_amount: maxAmount.value.trim() ? maxAmount.value.trim().replace(',', '.') : null,
    max_amount_currency: capCurrency.value || null,
  }))

  function save(onSaved: (id: string) => void) {
    const first = !source.workflowId()
    actions.save.mutate(
      { id: source.workflowId(), draft: draft.value },
      {
        onSuccess: (saved) => {
          savedSnapshot.value = snapshot()
          rememberSaved()
          draftOpened.value = false
          onSaved(saved.workflow.id)
          if (first && !saved.workflow.active && saved.issues.length === 0) offerToSwitchOn(saved.workflow.id)
        },
      },
    )
  }

  /** A new workflow is saved switched off: the next step, switching it on, is offered right away. */
  function offerToSwitchOn(id: string) {
    notify.info(t('workflow.enableNow.title'), {
      action: {
        label: t('workflow.enableNow.action'),
        onClick: () =>
          actions.setActive.mutate(
            { id, active: true },
            {
              onSuccess: (workflow) => {
                active.value = workflow.active
                savedSnapshot.value = snapshot()
                rememberSaved()
              },
            },
          ),
      },
    })
  }

  /** Throw the unsaved edits away: the canvas and the settings as they were saved. */
  function discardEdits() {
    const saved = savedState.value
    if (!saved) return
    ;[name.value, description.value, active.value, react.value, maxAmount.value] = saved.settings
    loadGraph(parseGraph(saved.graph))
    report.value = null
    draftOpened.value = false
    void nextTick(() => history.reset(graphJson.value))
  }

  /* --- a draft: edits kept aside without touching the saved workflow --- */
  const draftOpened = ref(false)
  const storedDraft = computed(() => (draftOpened.value ? null : (workflow.data.value?.draft ?? null)))

  /** Keep the current edits aside (an existing workflow only); the saved one keeps running. */
  function saveDraft(): Promise<void> {
    const id = source.workflowId()
    if (!id) return Promise.resolve()
    return new Promise((resolve, reject) => {
      actions.saveDraft.mutate(
        { id, graph: graphJson.value },
        {
          onSuccess: () => {
            // Kept: leaving now loses nothing.
            savedSnapshot.value = snapshot()
            resolve()
          },
          onError: reject,
        },
      )
    })
  }

  /** Put the kept-aside edits on the canvas; they count as unsaved until `Save`. */
  function openDraft() {
    const kept = workflow.data.value?.draft
    if (!kept) return
    loadGraph(parseGraph(kept.graph))
    draftOpened.value = true
    void nextTick(() => history.record(graphJson.value))
  }

  function dropDraft() {
    const id = source.workflowId()
    if (id) actions.discardDraft.mutate(id)
  }

  /** A version put back: the canvas shows it, and it counts as saved. */
  function onRestored(restored: Workflow) {
    active.value = restored.active
    loadGraph(parseGraph(restored.graph))
    report.value = null
    void nextTick(() => {
      savedSnapshot.value = snapshot()
      history.reset(graphJson.value)
    })
  }

  useEventListener(window, 'beforeunload', (event: BeforeUnloadEvent) => {
    if (dirty.value && !lesson.value) event.preventDefault()
  })

  return {
    workflow,
    lesson,
    name,
    description,
    active,
    react,
    maxAmount,
    capCurrency,
    loaded,
    dirty,
    history,
    storedDraft,
    save,
    discardEdits,
    saveDraft,
    openDraft,
    dropDraft,
    onRestored,
  }
}
