import { type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'

import { entityKey } from '@/shared/lib/entity-link'
import { formatDayMonthShort } from '@/shared/lib/format'

import { placeKey, useRecentPlaces } from '../../model/use-recent-places'
import type { PaletteRecentHit } from './use-palette-search'

const LIMIT = 5

export interface RecentRow {
  id: string
  title: string
  /** «кошелёк», «вчера», «операция». */
  note: string
  /** A card: the row opens it and the → / glance work on it. */
  entity?: EntityRef
  /** A screen with its filter. */
  path?: string
  wallet?: boolean
  at: string
}

/** Days back from today: 0 today, 1 yesterday. */
function daysAgo(iso: string): number {
  const day = (value: Date) => new Date(value.getFullYear(), value.getMonth(), value.getDate()).getTime()
  return Math.round((day(new Date()) - day(new Date(iso))) / 86_400_000)
}

/**
 * «Недавние» of an empty Ctrl K (Paths 1 · 7): wallets, screens with their
 * filter and the cards opened, in one list by time, without the screen one is on.
 */
export function usePaletteRecent(entities: Ref<PaletteRecentHit[]>) {
  const { t } = useI18n()
  const route = useRoute()
  const places = useRecentPlaces()

  const when = (at: string | undefined) => {
    if (!at) return ''
    const days = daysAgo(at)
    return days <= 0 ? t('palette.today') : days === 1 ? t('palette.yesterday') : formatDayMonthShort(at)
  }

  return computed<RecentRow[]>(() => {
    const screens = places.value
      .filter((place) => placeKey(place.path) !== route.path)
      .map((place) => {
        const wallet = /^\/stores\/[^/]+$/.test(placeKey(place.path))
        const note = wallet ? t('palette.kind.wallet') : when(place.at)
        return { id: `place-${place.path}`, title: place.title, note, path: place.path, wallet, at: place.at ?? '' }
      })
    const cards = entities.value.map((hit) => ({
      id: hit.id,
      title: hit.label,
      note: t(`palette.kind.${hit.target.kind}`),
      entity: hit.target,
      at: hit.at ?? '',
    }))
    const seen = new Set<string>()
    const rows: RecentRow[] = [...screens, ...cards]
    return rows
      .sort((left, right) => right.at.localeCompare(left.at))
      .filter((row) => {
        const key = row.entity ? entityKey(row.entity) : row.id
        if (seen.has(key)) return false
        seen.add(key)
        return true
      })
      .slice(0, LIMIT)
  })
}
