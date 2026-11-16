import { afterEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'

import { clearRecentPlaces, useRecentPlaces, useRecordPlaces } from './use-recent-places'

vi.mock('vue-router', () => ({
  useRoute: () => ({ fullPath: '/money?month=10', path: '/money', name: 'money', params: {}, query: {} }),
}))
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('../lib/place-title', () => ({ placeTitle: (_route: unknown, name: string) => name }))

afterEach(() => {
  clearRecentPlaces()
  vi.restoreAllMocks()
})

describe('recent places', () => {
  it('are recorded in memory without touching localStorage', () => {
    const spy = vi.spyOn(Storage.prototype, 'setItem')
    useRecordPlaces(ref('Money'))
    expect(useRecentPlaces().value).toHaveLength(1)
    expect(spy).not.toHaveBeenCalled()
  })

  it('are empty after clearRecentPlaces', () => {
    useRecordPlaces(ref('Money'))
    clearRecentPlaces()
    expect(useRecentPlaces().value).toEqual([])
  })
})
