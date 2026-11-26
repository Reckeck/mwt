import { computed, ref } from 'vue'

import { parseError } from '@/shared/api/ipc'

import type { PagePick } from '../ui/page-pick.types'
import type { SourceDraftState } from './use-source-draft'
import type { useSourceActions } from './use-source-actions'

/**
 * `Open` the address and turn a click in the preview into the method:
 * an element or a table cell on a page, a path in JSON, a column in a file.
 */
export function useSourcePick(state: SourceDraftState, actions: ReturnType<typeof useSourceActions>) {
  const preview = ref<SourcePreview | null>(null)
  const previewError = ref<string | null>(null)
  const result = ref<Extracted | null>(null)
  const testError = ref<string | null>(null)

  async function open() {
    previewError.value = null
    preview.value = null
    try {
      const page = await actions.preview.mutateAsync(state.url.value.trim())
      preview.value = page
      if (page.kind === 'json' && state.kind.value !== 'json') state.kind.value = 'json'
      if ((page.kind === 'csv' || page.kind === 'xlsx') && state.kind.value !== 'file') {
        state.kind.value = 'file'
        state.fileFormat.value = page.kind
      }
    } catch (err) {
      previewError.value = parseError(err).message
    }
  }

  function pickElement(pick: PagePick) {
    if (pick.table?.header) {
      state.kind.value = 'table'
      state.selector.value = pick.table.selector
      state.column.value = pick.table.header
    } else {
      state.kind.value = 'css'
      state.selector.value = pick.selector
      state.textHint.value = pick.hint
    }
    result.value = null
  }

  function pickPath(path: string, isArray: boolean) {
    state.kind.value = 'json'
    state.path.value = path
    if (isArray) state.output.value = 'series'
    result.value = null
  }

  function pickColumn(column: string) {
    state.column.value = column
    result.value = null
  }

  async function test() {
    testError.value = null
    result.value = null
    try {
      result.value = await actions.test.mutateAsync(state.toDraft())
    } catch (err) {
      testError.value = parseError(err).message
    }
  }

  function reset() {
    preview.value = null
    previewError.value = null
    result.value = null
    testError.value = null
  }

  /** The selector the page picker marks: the table's for a table, the element's otherwise. */
  const marked = computed(() =>
    state.kind.value === 'css' || state.kind.value === 'table' ? state.selector.value : '',
  )

  return { preview, previewError, result, testError, marked, open, pickElement, pickPath, pickColumn, test, reset }
}
