// check-ui-ignore invalidate-all: the whole document was replaced by the JSON editor
import { type Ref, computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { closeCompletion, currentCompletions, startCompletion } from '@codemirror/autocomplete'

import { type AppError, call, parseError } from '@/shared/api/ipc'
import { vaultJsonEditor } from '@/shared/lib/editor'
import { normalizeJson, parseJson } from '@/shared/lib/json'
import { type JsonSchema, JsonSchemaReader } from '@/shared/lib/json-schema-reader'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'

/** Idle time after the last keystroke before the draft is revalidated. */
const VALIDATE_DEBOUNCE_MS = 400
/** Idle time before the completion popup offers itself unprompted. */
const AUTO_OPEN_DELAY_MS = 1500

export type VaultJsonStatus = 'loading' | 'invalid' | 'rejected' | 'applying' | 'pending' | 'saved'

/**
 * The raw-JSON view of the vault: a CodeMirror draft that never writes on its
 * own. Ctrl+S is the only thing that persists, and a draft that does not parse
 * is refused outright. Typing revalidates after a pause, so the status tracks
 * the draft without any write. `$EDITOR` round trips through a scratch file.
 */
export function useVaultJsonEditor(mountEl: Ref<HTMLDivElement | null>) {
  const queryClient = useQueryClient()
  const settingsStore = useSettingsStore()

  let view: EditorView | null = null
  const appliedRaw = ref('')
  /** `normalizeJson(appliedRaw)` — the "nothing to save" baseline. */
  const appliedNormalized = ref<string | null>(null)
  const syntaxError = ref<string | null>(null)
  const applyError = ref<AppError | null>(null)
  const scratchPath = ref<string | null>(null)
  const loading = ref(true)
  const applying = ref(false)
  /** Whether the draft parses and still differs from what is on disk. */
  const dirty = ref(false)

  const status = computed<VaultJsonStatus>(() => {
    if (loading.value) return 'loading'
    if (syntaxError.value) return 'invalid'
    if (applyError.value) return 'rejected'
    if (applying.value) return 'applying'
    return dirty.value ? 'pending' : 'saved'
  })

  let validateTimer: ReturnType<typeof setTimeout> | undefined
  let autoOpenTimer: ReturnType<typeof setTimeout> | undefined
  const cancelPending = () => {
    clearTimeout(validateTimer)
    validateTimer = undefined
  }
  const cancelAutoOpen = () => {
    clearTimeout(autoOpenTimer)
    autoOpenTimer = undefined
  }
  /** Offer the completion list once typing stops, unless it is already up. */
  function scheduleAutoOpen() {
    cancelAutoOpen()
    autoOpenTimer = setTimeout(() => {
      autoOpenTimer = undefined
      // An open list is not re-opened: that would re-place it under a caret the user may have nudged.
      if (view && !currentCompletions(view.state)) startCompletion(view)
    }, AUTO_OPEN_DELAY_MS)
  }

  /** A failed request: kept for the status badge, and said in a toast with the reason. */
  function failed(err: unknown, titleKey: string) {
    applyError.value = parseError(err)
    notify.error(err, titleKey)
  }

  /** Persist the current draft. Only Ctrl+S gets here. */
  async function save() {
    const editor = view
    if (!editor || applying.value) return
    const text = editor.state.doc.toString()
    const parsed = parseJson(text)
    if (!parsed.ok) {
      // Refuse before touching the vault: a malformed document must never overwrite what is stored.
      syntaxError.value = parsed.error
      return
    }
    applying.value = true
    applyError.value = null
    syntaxError.value = null
    // Invalidating every query re-renders the settings page, and the dialog's focus trap
    // pulls focus to itself when its subtree is patched: remember where the caret was.
    const hadFocus = editor.hasFocus
    const head = editor.state.selection.main.head
    try {
      const stored = await call<string>('dev_apply_vault_json', { json: text })
      appliedRaw.value = stored
      // The backend re-serializes canonically, so the baseline is its output, not the text sent.
      appliedNormalized.value = normalizeJson(stored)
      dirty.value = normalizeJson(editor.state.doc.toString()) !== appliedNormalized.value
      // The whole document may have changed, so every cached view is suspect.
      await queryClient.invalidateQueries()
      await settingsStore.loadVault()
      if (view && hadFocus) {
        view.focus()
        view.dispatch({ selection: { anchor: Math.min(head, view.state.doc.length) } })
      }
      notify.success('settings.dev.jsonApplied')
    } catch (err) {
      failed(err, 'settings.dev.jsonRejected')
    } finally {
      applying.value = false
    }
  }

  function resetToApplied() {
    cancelPending()
    if (!view) return
    closeCompletion(view)
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: appliedRaw.value } })
    view.focus()
  }

  async function openInEditor() {
    cancelPending()
    try {
      scratchPath.value = await call<string>('dev_open_vault_json_in_editor')
      notify.success('settings.dev.openedInEditor', { description: scratchPath.value })
    } catch (err) {
      failed(err, 'settings.dev.openInEditorFailed')
    }
  }

  async function reloadFromEditor() {
    cancelPending()
    try {
      const text = await call<string>('dev_reload_vault_json_from_editor')
      view?.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } })
      view?.focus()
      scheduleAutoOpen()
      notify.success('settings.dev.reloadedFromEditor')
    } catch (err) {
      failed(err, 'settings.dev.reloadFailed')
    }
  }

  /** Editing never writes: it revalidates after a pause so the badge reflects the draft. */
  const revalidate = EditorView.updateListener.of((update) => {
    if (!update.docChanged) return
    cancelAutoOpen()
    closeCompletion(update.view)
    scheduleAutoOpen()
    cancelPending()
    validateTimer = setTimeout(() => {
      validateTimer = undefined
      const text = update.state.doc.toString()
      const parsed = parseJson(text)
      syntaxError.value = parsed.error
      const normalized = parsed.ok ? normalizeJson(text) : null
      dirty.value = normalized !== null && normalized !== appliedNormalized.value
    }, VALIDATE_DEBOUNCE_MS)
  })

  onMounted(async () => {
    const parent = mountEl.value
    if (!parent) return
    // The document shape comes from the backend, so the editor is built once the schema arrives.
    let reader: JsonSchemaReader
    try {
      reader = new JsonSchemaReader(await call<JsonSchema>('vault_schema'))
    } catch (err) {
      failed(err, 'settings.dev.schemaFailed')
      loading.value = false
      return
    }
    view = new EditorView({
      state: EditorState.create({ doc: '', extensions: [vaultJsonEditor(reader, () => void save()), revalidate] }),
      parent,
    })
    try {
      const result = await call<DevDumpVaultOutput>('dev_dump_vault')
      appliedRaw.value = result.json
      appliedNormalized.value = normalizeJson(result.json)
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: result.json } })
    } catch (err) {
      failed(err, 'settings.dev.loadFailed')
    } finally {
      loading.value = false
    }
  })

  onBeforeUnmount(() => {
    cancelPending()
    cancelAutoOpen()
    view?.destroy()
    view = null
  })

  return { status, syntaxError, scratchPath, loading, resetToApplied, openInEditor, reloadFromEditor }
}
