import { describe, expect, it } from 'vitest'

import { BOTTOM_FAB_CLASS, BOTTOM_FAB_CLEARANCE_CLASS } from './bottom-fab'

describe('bottom FAB', () => {
  it('is a 3.75rem circle, 1rem from the right and 1rem above the bar', () => {
    expect(BOTTOM_FAB_CLASS).toBe('absolute right-4 bottom-full mb-4 size-15 rounded-full')
  })

  it('page clearance matches the box and both gaps', () => {
    expect(BOTTOM_FAB_CLEARANCE_CLASS).toBe('pb-[calc(1rem+3.75rem+1rem)]')
  })
})
