<script setup lang="ts">
/// Read/write raw JSON view of the unlocked vault document.
///
/// Editing model: the editor holds a *draft* and **never writes on its own**.
/// Ctrl+S is the only thing that persists, and a draft that does not parse is
/// refused outright — the document on disk keeps whatever was saved last, and
/// the status line says so. Typing merely revalidates the draft after a pause,
/// so the "in sync / not in sync" badge tracks reality without any write.
///
/// The editor itself is CodeMirror (see `lib/editor.ts`), which owns
/// highlighting, undo history, bracket handling, folding and the completion
/// popup. The component only wires it to the IPC layer.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueryClient } from '@tanstack/vue-query'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { closeCompletion, currentCompletions, startCompletion } from '@codemirror/autocomplete'
import { Check, ExternalLink, RotateCcw, Undo2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { EDITOR_HEIGHT, vaultJsonEditor } from '@/shared/lib/editor'
import { normalizeJson, parseJson } from '@/shared/lib/json'
import { JsonSchemaReader, type JsonSchema } from '@/shared/lib/json-schema-reader'
import { useSettingsStore } from '@/shared/store/settings'

/** Idle time after the last keystroke before the draft is revalidated. */
const VALIDATE_DEBOUNCE_MS = 400

/** Idle time before the completion popup offers itself unprompted. */
const AUTO_OPEN_DELAY_MS = 1500

const { t } = useI18n()
const queryClient = useQueryClient()
const settingsStore = useSettingsStore()

const mountEl = ref<HTMLDivElement | null>(null)
let view: EditorView | null = null

const appliedRaw = ref('')
/** `normalizeJson(appliedRaw)` — the "nothing to save" baseline. */
const appliedNormalized = ref<string | null>(null)
const syntaxError = ref<string | null>(null)
const applyError = ref<AppError | null>(null)
const scratchPath = ref<string | null>(null)
const loading = ref(true)
const applying = ref(false)

let validateTimer: ReturnType<typeof setTimeout> | undefined
let autoOpenTimer: ReturnType<typeof setTimeout> | undefined

/** Whether the draft parses and still differs from what is on disk. */
const dirtyFlag = ref(false)

const status = computed(() => {
  if (loading.value) return 'loading'
  if (syntaxError.value) return 'invalid'
  if (applyError.value) return 'rejected'
  if (applying.value) return 'applying'
  return dirtyFlag.value ? 'pending' : 'saved'
})

/** Localize a backend error by code, falling back to the raw message. */
function errorText(err: AppError): string {
  const known = t(`errors.${err.code}`)
  return known === `errors.${err.code}` ? err.message : known
}

/**
 * Syntax errors stay inline: they track the caret while typing and must be read
 * next to the draft, not as a toast that vanishes. A backend rejection is the
 * opposite — a finished request with a definitive answer, so it is a toast.
 */
const noticeText = computed(() =>
  syntaxError.value ? `${t('settings.dev.jsonInvalid')}: ${syntaxError.value}` : '',
)

function cancelPending() {
  if (validateTimer !== undefined) {
    clearTimeout(validateTimer)
    validateTimer = undefined
  }
}

function cancelAutoOpen() {
  if (autoOpenTimer !== undefined) {
    clearTimeout(autoOpenTimer)
    autoOpenTimer = undefined
  }
}

/** Offer the completion list once typing stops, unless it is already up. */
function scheduleAutoOpen() {
  cancelAutoOpen()
  autoOpenTimer = setTimeout(() => {
    autoOpenTimer = undefined
    if (!view) return
    // Do not re-open an open list: that would re-place it under a caret the
    // user may have nudged in the meantime.
    if (currentCompletions(view.state)) return
    startCompletion(view)
  }, AUTO_OPEN_DELAY_MS)
}

/** Persist the current draft. Only Ctrl+S gets here. */
async function save() {
  const editor = view
  if (!editor || applying.value) return

  const text = editor.state.doc.toString()
  const parsed = parseJson(text)
  if (!parsed.ok) {
    // Refuse before touching the vault: a malformed document must never
    // overwrite what is already stored.
    syntaxError.value = parsed.error
    return
  }

  applying.value = true
  applyError.value = null
  syntaxError.value = null
  // Invalidating every query re-renders the settings page, and the dialog's
  // focus trap pulls focus to itself when its subtree is patched. Remember
  // where the caret was so the editor does not lose it on every save.
  const hadFocus = editor.hasFocus
  const head = editor.state.selection.main.head

  try {
    const stored = await call<string>('dev_apply_vault_json', { json: text })
    appliedRaw.value = stored
    // The backend re-serializes canonically, so the baseline has to be its
    // output, not the text we happened to send.
    appliedNormalized.value = normalizeJson(stored)
    dirtyFlag.value = normalizeJson(editor.state.doc.toString()) !== appliedNormalized.value
    // The whole document may have changed, so every cached view is suspect.
    await queryClient.invalidateQueries()
    await settingsStore.loadVault()
    if (view && hadFocus) {
      view.focus()
      view.dispatch({ selection: { anchor: Math.min(head, view.state.doc.length) } })
    }
    toast.success(t('settings.dev.jsonApplied'))
  } catch (err) {
    const parsed = parseError(err)
    applyError.value = parsed
    toast.error(t('settings.dev.jsonRejected'), { description: errorText(parsed) })
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
    // Only the file the backend hands to $EDITOR; nothing to pass back.
    scratchPath.value = await call<string>('dev_open_vault_json_in_editor')
    toast.success(t('settings.dev.openedInEditor'), { description: scratchPath.value })
  } catch (err) {
    const parsed = parseError(err)
    applyError.value = parsed
    toast.error(t('settings.dev.openInEditorFailed'), { description: errorText(parsed) })
  }
}

async function reloadFromEditor() {
  cancelPending()
  try {
    const text = await call<string>('dev_reload_vault_json_from_editor')
    view?.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } })
    view?.focus()
    scheduleAutoOpen()
    toast.success(t('settings.dev.reloadedFromEditor'))
  } catch (err) {
    const parsed = parseError(err)
    applyError.value = parsed
    toast.error(t('settings.dev.reloadFailed'), { description: errorText(parsed) })
  }
}

onMounted(async () => {
  const parent = mountEl.value
  if (!parent) return

  // The document shape comes from the backend, so the editor can only be built
  // once the schema has arrived.
  let reader: JsonSchemaReader
  try {
    reader = new JsonSchemaReader(await call<JsonSchema>('vault_schema'))
  } catch (err) {
    const parsed = parseError(err)
    applyError.value = parsed
    toast.error(t('settings.dev.schemaFailed'), { description: errorText(parsed) })
    loading.value = false
    return
  }

  const state = EditorState.create({
    doc: '',
    extensions: [
      vaultJsonEditor(reader, () => void save()),
      // Editing never writes. It revalidates after a pause so the status badge
      // reflects the draft, and a draft that fails to parse is simply not
      // saveable.
      EditorView.updateListener.of((update) => {
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
          dirtyFlag.value = normalized !== null && normalized !== appliedNormalized.value
        }, VALIDATE_DEBOUNCE_MS)
      }),
    ],
  })

  view = new EditorView({ state, parent })

  try {
    const result = await call<DevDumpVaultOutput>('dev_dump_vault')
    appliedRaw.value = result.json
    appliedNormalized.value = normalizeJson(result.json)
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: result.json } })
  } catch (err) {
    const parsed = parseError(err)
    applyError.value = parsed
    toast.error(t('settings.dev.loadFailed'), { description: errorText(parsed) })
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
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <Badge :variant="status === 'saved' ? 'default' : 'secondary'">
        <span class="flex items-center gap-1">
          <Check v-if="status === 'saved'" class="size-3" />
          {{ t(`settings.dev.status.${status}`) }}
        </span>
      </Badge>
      <span class="flex-1" />
      <Button variant="ghost" size="sm" :disabled="loading" @click="openInEditor">
        <ExternalLink class="size-4" />
        {{ t('settings.dev.openInEditor') }}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        :disabled="loading || !scratchPath"
        :title="t('settings.dev.reloadHint')"
        @click="reloadFromEditor"
      >
        <Undo2 class="size-4" />
        {{ t('settings.dev.reloadFromEditor') }}
      </Button>
      <Button variant="ghost" size="sm" :disabled="loading" @click="resetToApplied">
        <RotateCcw class="size-4" />
        {{ t('settings.dev.resetDraft') }}
      </Button>
    </div>

    <p v-if="noticeText" class="text-xs text-destructive">{{ noticeText }}</p>
    <p v-if="scratchPath" class="break-all text-xs text-muted-foreground">
      {{ t('settings.dev.scratchFile') }}: {{ scratchPath }}
    </p>

    <div
      ref="mountEl"
      class="overflow-hidden rounded-md border border-border bg-muted/50 font-mono"
      :style="{ height: EDITOR_HEIGHT }"
    />

    <p class="text-xs text-muted-foreground">{{ t('settings.dev.saveHint') }}</p>
    <p class="text-xs text-muted-foreground">{{ t('settings.dev.autocompleteHint') }}</p>
  </div>
</template>