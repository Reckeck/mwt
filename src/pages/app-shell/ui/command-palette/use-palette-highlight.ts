import { type Ref, ref } from 'vue'
import { useMutationObserver } from '@vueuse/core'

import { parseEntityKey } from '@/shared/lib/entity-link'

export interface PaletteHighlight {
  target: EntityRef
  title: string
}

/** The card under the palette's highlight, from its row's `data-entity`: what the glance shows. */
export function usePaletteHighlight(list: Ref<HTMLElement | null>) {
  const current = ref<PaletteHighlight | null>(null)
  function read() {
    const row = list.value?.querySelector<HTMLElement>('[data-highlighted][data-entity]')
    const target = row?.dataset.entity ? parseEntityKey(row.dataset.entity) : null
    const same =
      current.value && target && current.value.target.kind === target.kind && current.value.target.id === target.id
    if (same) return
    current.value = target ? { target, title: row?.dataset.title ?? '' } : null
  }
  useMutationObserver(list, read, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ['data-highlighted'],
  })
  return current
}
