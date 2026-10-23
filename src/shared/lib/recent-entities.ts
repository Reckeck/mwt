import { ref } from 'vue'

import { entityKey, parseEntityKey, sameEntity } from './entity-link'

const STORAGE_KEY = 'palette.recent'
const RECENT_CAP = 10

/** A card opened lately: when, and the title it had («Маме до зарплаты · 20 окт»). */
export interface RecentEntity {
  target: EntityRef
  /** ISO; older records have none. */
  at?: string
  title?: string
}

interface Stored {
  key: string
  at?: string
  title?: string
}

function load(): RecentEntity[] {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]')
    if (!Array.isArray(raw)) return []
    // Older versions kept the bare keys.
    return raw.flatMap((entry: unknown) => {
      const stored: Stored | null =
        typeof entry === 'string' ? { key: entry } : entry && typeof entry === 'object' ? (entry as Stored) : null
      const target = stored ? parseEntityKey(stored.key) : null
      return target ? [{ target, at: stored?.at, title: stored?.title }] : []
    })
  } catch {
    return []
  }
}

/** The last cards opened on this device, newest first: a convenience for the palette, not vault data. */
export const recentEntities = ref<RecentEntity[]>(load())

/** Moves the card to the top; a title given later (once the card loads) fills in the same entry. */
export function rememberEntity(target: EntityRef, title?: string) {
  const known = recentEntities.value.find((entry) => sameEntity(entry.target, target))
  const fresh: RecentEntity = { target, at: new Date().toISOString(), title: title ?? known?.title }
  const next = [fresh, ...recentEntities.value.filter((entry) => !sameEntity(entry.target, target))].slice(
    0,
    RECENT_CAP,
  )
  recentEntities.value = next
  try {
    const stored: Stored[] = next.map((entry) => ({ key: entityKey(entry.target), at: entry.at, title: entry.title }))
    localStorage.setItem(STORAGE_KEY, JSON.stringify(stored))
  } catch {
    // Private mode or blocked storage: the list lives until the app closes.
  }
}
