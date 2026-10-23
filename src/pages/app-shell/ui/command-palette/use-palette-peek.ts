import { type Ref, computed, ref, watch } from 'vue'
import { onLongPress, useEventListener } from '@vueuse/core'

import { parseEntityKey } from '@/shared/lib/entity-link'

import { type PaletteHighlight, usePaletteHighlight } from './use-palette-highlight'

/** Kinds the glance can show. */
const PEEKABLE = new Set<EntityKind>(['category', 'store'])

/**
 * When the glance beside the results shows (Paths 2 · 8): on its own while a
 * query is typed, on Shift Enter for an empty one, and on a phone after a long
 * press on a row — then it stays until the query changes.
 */
export function usePalettePeek(options: {
  open: Ref<boolean>
  search: Ref<string>
  list: Ref<HTMLElement | null>
  phone: Ref<boolean>
}) {
  const highlight = usePaletteHighlight(options.list)
  const forced = ref(false)
  const held = ref<PaletteHighlight | null>(null)

  watch([options.search, options.open], () => {
    forced.value = false
    held.value = null
  })

  // Capture: Shift Enter must not reach the list, which would open the row.
  useEventListener(
    window,
    'keydown',
    (event: KeyboardEvent) => {
      if (!options.open.value || event.key !== 'Enter' || !event.shiftKey) return
      event.preventDefault()
      event.stopPropagation()
      forced.value = true
    },
    { capture: true },
  )

  onLongPress(
    options.list,
    (event) => {
      const row = (event.target as HTMLElement | null)?.closest<HTMLElement>('[data-entity]')
      const target = row?.dataset.entity ? parseEntityKey(row.dataset.entity) : null
      if (target) held.value = { target, title: row?.dataset.title ?? '' }
    },
    { delay: 500 },
  )

  const item = computed(() => {
    if (options.phone.value) return held.value
    const current = highlight.value
    if (!current || !PEEKABLE.has(current.target.kind)) return null
    return options.search.value.trim() || forced.value ? current : null
  })
  const onCategory = computed(() => highlight.value?.target.kind === 'category')

  return { item, onCategory, close: () => (held.value = null) }
}
