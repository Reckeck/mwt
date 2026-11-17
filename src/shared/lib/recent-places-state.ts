import { type Ref, readonly, ref } from 'vue'

/** A place the user opened: the address to come back to, with its filter, and the title it had then. */
export interface RecentPlace {
  /** The full path: `Money › Spending · October` comes back to October (Paths 1 · 7). */
  path: string
  title: string
  /** When it was last open, ISO. */
  at?: string
}

/** This session's list (Shell · 4); titles and filters are vault content, so it lives in memory only. */
const places = ref<RecentPlace[]>([])

/** The places, read-only: only `setRecentPlaces` and `clearRecentPlaces` change them. */
export const recentPlaces: Readonly<Ref<readonly RecentPlace[]>> = readonly(places)

export function setRecentPlaces(next: RecentPlace[]): void {
  places.value = next
}

export function clearRecentPlaces(): void {
  places.value = []
}
