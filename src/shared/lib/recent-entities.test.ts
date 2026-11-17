import { afterEach, describe, expect, it, vi } from 'vitest'

import { clearRecentEntities, recentEntities, rememberEntity } from './recent-entities'

afterEach(() => {
  clearRecentEntities()
  vi.restoreAllMocks()
})

describe('recent entities', () => {
  it('stay in memory: nothing is written to localStorage', () => {
    const spy = vi.spyOn(Storage.prototype, 'setItem')
    rememberEntity({ kind: 'category', id: 't1' }, 'To Mom')
    expect(recentEntities.value).toHaveLength(1)
    expect(spy).not.toHaveBeenCalled()
  })

  it('are empty after clearRecentEntities', () => {
    rememberEntity({ kind: 'category', id: 't1' })
    clearRecentEntities()
    expect(recentEntities.value).toEqual([])
  })
})
