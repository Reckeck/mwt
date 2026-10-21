import { ref } from 'vue'

import { entityKey, parseEntityKey, sameEntity } from './entity-link'

const STORAGE_KEY = 'palette.recent'
const RECENT_CAP = 10

function load(): EntityRef[] {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]')
    if (!Array.isArray(raw)) return []
    return raw.flatMap((key) => (typeof key === 'string' ? (parseEntityKey(key) ?? []) : []))
  } catch {
    return []
  }
}

/** The last cards opened on this device, newest first: a convenience for the palette, not vault data. */
export const recentEntities = ref<EntityRef[]>(load())

export function rememberEntity(target: EntityRef) {
  const next = [target, ...recentEntities.value.filter((other) => !sameEntity(other, target))].slice(0, RECENT_CAP)
  recentEntities.value = next
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next.map(entityKey)))
  } catch {
    // Private mode or blocked storage: the list lives until the app closes.
  }
}
