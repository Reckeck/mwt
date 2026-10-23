import { computed, readonly, ref, watch } from 'vue'
import type { ComputedRef, DeepReadonly, Ref } from 'vue'

/**
 * Rows picked in a list (Ops-1 · 4): a click toggles one, Shift extends from
 * the last one clicked in the list's order, a day header takes its rows.
 * Rows that left the list (a new filter, a delete) leave the selection too.
 */
export interface Selection {
  ids: DeepReadonly<Ref<Set<string>>>
  /** Something is picked: the list shows its marks, the page its bar. */
  active: ComputedRef<boolean>
  count: ComputedRef<number>
  has(id: string): boolean
  toggle(id: string, range?: boolean): void
  /** Pick or drop every id of a group (a day). */
  setMany(ids: readonly string[], on: boolean): void
  clear(): void
}

export function useSelection(order: () => readonly string[]): Selection {
  const ids = ref(new Set<string>())
  let anchor: string | null = null

  watch(order, (now) => {
    const known = new Set(now)
    const kept = [...ids.value].filter((id) => known.has(id))
    if (kept.length !== ids.value.size) ids.value = new Set(kept)
  })

  function toggle(id: string, range = false) {
    const next = new Set(ids.value)
    const list = order()
    const from = anchor ? list.indexOf(anchor) : -1
    const to = list.indexOf(id)
    if (range && from >= 0 && to >= 0) {
      // Shift takes the anchor's state to every row between.
      const on = next.has(anchor as string)
      for (const each of list.slice(Math.min(from, to), Math.max(from, to) + 1)) {
        if (on) next.add(each)
        else next.delete(each)
      }
    } else if (next.has(id)) next.delete(id)
    else next.add(id)
    anchor = id
    ids.value = next
  }

  function setMany(group: readonly string[], on: boolean) {
    const next = new Set(ids.value)
    for (const id of group) {
      if (on) next.add(id)
      else next.delete(id)
    }
    ids.value = next
  }

  return {
    ids: readonly(ids),
    active: computed(() => ids.value.size > 0),
    count: computed(() => ids.value.size),
    has: (id) => ids.value.has(id),
    toggle,
    setMany,
    clear: () => {
      ids.value = new Set()
      anchor = null
    },
  }
}
