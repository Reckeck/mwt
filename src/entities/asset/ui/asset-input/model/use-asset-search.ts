import { onScopeDispose, ref } from 'vue'

import { call } from '@/shared/api/ipc'

/** How long typing pauses before the backend search runs, ms. */
const SEARCH_DELAY_MS = 250

/**
 * The backend asset search behind a text field: debounced, and only the
 * answer to the latest query is kept (an older, slower one is dropped).
 */
export function useAssetSearch() {
  const hits = ref<AssetSearchHit[]>([])
  const loading = ref(false)
  let timer: ReturnType<typeof setTimeout> | undefined
  let seq = 0

  async function search(rawQuery: string) {
    const token = ++seq
    if (rawQuery.trim().length < 1) {
      hits.value = []
      loading.value = false
      return
    }
    loading.value = true
    try {
      const res = await call<AssetSearchHit[]>('search_assets', { query: rawQuery.trim() })
      if (token === seq) hits.value = res
    } catch {
      if (token === seq) hits.value = []
    } finally {
      if (token === seq) loading.value = false
    }
  }

  /** Search for `query` once typing pauses. */
  function schedule(query: string) {
    clearTimeout(timer)
    timer = setTimeout(() => void search(query), SEARCH_DELAY_MS)
  }

  function clear() {
    clearTimeout(timer)
    seq++
    hits.value = []
    loading.value = false
  }

  onScopeDispose(() => clearTimeout(timer))

  return { hits, loading, schedule, clear }
}
