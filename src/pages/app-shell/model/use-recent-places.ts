import { type Ref, watch } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { placeTitle } from '../lib/place-title'

/** A place the user opened: the address to come back to, with its filter, and the title it had then. */
export interface RecentPlace {
  /** The full path: «Деньги › Траты · октябрь» comes back to October (Paths 1 · 7). */
  path: string
  title: string
  /** When it was last open, ISO; older records have none. */
  at?: string
}

const LIMIT = 5
/** The device's own list (Shell · 4: an empty search shows where one was last). */
const places = useLocalStorage<RecentPlace[]>('trade-app:recent-places', [])

/** The screen without its query: one place per screen, the newest filter wins. */
export const placeKey = (path: string) => path.split('?')[0] ?? path

/** The shell records each screen under its title; the same screen moves to the top. */
export function useRecordPlaces(title: Ref<string>): void {
  const route = useRoute()
  const { t } = useI18n()
  watch(
    () => [route.fullPath, title.value] as const,
    ([path, name]) => {
      if (!name || route.name === 'dashboard') return
      const fresh = { path, title: placeTitle(route, name, t), at: new Date().toISOString() }
      places.value = [fresh, ...places.value.filter((place) => placeKey(place.path) !== route.path)].slice(0, LIMIT)
    },
    { immediate: true },
  )
}

/** The places, newest first. */
export function useRecentPlaces(): Readonly<Ref<RecentPlace[]>> {
  return places
}
