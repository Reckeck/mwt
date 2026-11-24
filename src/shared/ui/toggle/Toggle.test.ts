import { describe, expect, it } from 'vitest'

import { toggleVariants } from '.'

describe('toggleVariants', () => {
  it('chip is a pill that turns ink when on', () => {
    const cls = toggleVariants({ variant: 'chip' })
    expect(cls).toContain('rounded-full')
    expect(cls).toContain('data-[state=on]:bg-primary')
    expect(cls).toContain('color-mix(')
  })

  it('keeps the existing variants', () => {
    for (const variant of ['default', 'outline', 'segmented'] as const) {
      expect(toggleVariants({ variant })).toContain('touch-min')
    }
  })
})
