import { afterEach, expect, it } from 'vitest'

import { purgeLegacyRecents } from './purge-legacy-recents'

afterEach(() => localStorage.clear())

it('removes both legacy recents keys and leaves other keys', () => {
  localStorage.setItem('palette.recent', '[]')
  localStorage.setItem('trade-app:recent-places', '[]')
  localStorage.setItem('chart.currency', 'USD')
  purgeLegacyRecents()
  expect(localStorage.getItem('palette.recent')).toBeNull()
  expect(localStorage.getItem('trade-app:recent-places')).toBeNull()
  expect(localStorage.getItem('chart.currency')).toBe('USD')
})
