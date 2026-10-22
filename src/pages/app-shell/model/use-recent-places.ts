import { type Ref, watch } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { useRoute } from 'vue-router'

/** A place the user opened: the path to come back to and the title it had then. */
export interface RecentPlace {
  path: string
  title: string
}

const LIMIT = 5
/** The device's own list (Shell · 4: an empty search shows where one was last). */
const places = useLocalStorage<RecentPlace[]>('trade-app:recent-places', [])

/** The shell records each screen under its title; the same screen moves to the top. */
export function useRecordPlaces(title: Ref<string>): void {
  const route = useRoute()
  watch(
    () => [route.path, title.value] as const,
    ([path, name]) => {
      if (!name || route.name === 'dashboard') return
      places.value = [{ path, title: name }, ...places.value.filter((place) => place.path !== path)].slice(0, LIMIT)
    },
    { immediate: true },
  )
}

/** The places, newest first, but the one open now. */
export function useRecentPlaces(): Readonly<Ref<RecentPlace[]>> {
  return places
}
