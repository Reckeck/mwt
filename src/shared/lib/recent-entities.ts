import { ref } from 'vue'

import { sameEntity } from './entity-link'

const RECENT_CAP = 10

/** A card opened lately: when, and the title it had (`To Mom before payday · Oct 20`). */
export interface RecentEntity {
  target: EntityRef
  /** ISO. */
  at?: string
  title?: string
}

/**
 * The last cards opened in this unlocked session, newest first: a convenience for the palette.
 * Titles are vault content, so the list lives in memory only and is emptied when the vault closes.
 */
export const recentEntities = ref<RecentEntity[]>([])

/** Moves the card to the top; a title given later (once the card loads) fills in the same entry. */
export function rememberEntity(target: EntityRef, title?: string) {
  const known = recentEntities.value.find((entry) => sameEntity(entry.target, target))
  const fresh: RecentEntity = { target, at: new Date().toISOString(), title: title ?? known?.title }
  recentEntities.value = [fresh, ...recentEntities.value.filter((entry) => !sameEntity(entry.target, target))].slice(
    0,
    RECENT_CAP,
  )
}

export function clearRecentEntities() {
  recentEntities.value = []
}
