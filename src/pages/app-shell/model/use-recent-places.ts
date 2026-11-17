import { type Ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { type RecentPlace, clearRecentPlaces, recentPlaces, setRecentPlaces } from '@/shared/lib/recent-places-state'

import { placeTitle } from '../lib/place-title'

export type { RecentPlace }
export { clearRecentPlaces }

const LIMIT = 5

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
      setRecentPlaces(
        [fresh, ...recentPlaces.value.filter((place) => placeKey(place.path) !== route.path)].slice(0, LIMIT),
      )
    },
    { immediate: true },
  )
}

/** The places, newest first. */
export function useRecentPlaces(): Readonly<Ref<readonly RecentPlace[]>> {
  return recentPlaces
}
