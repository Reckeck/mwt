import { describe, expect, it } from 'vitest'

import { runToneClass, statusTone } from './card-view'

describe('run tone', () => {
  it('colours the outcomes', () => {
    expect(runToneClass('done')).toBe('text-success')
    expect(runToneClass('asking')).toBe('text-warning')
    expect(runToneClass('failed')).toBe('text-destructive')
    expect(runToneClass('shown')).toBe('text-muted-foreground')
  })

  it('shows a waiting run as needing attention, in one place for card and journal', () => {
    expect(statusTone('waiting')).toBe('wrn')
    expect(runToneClass('waiting')).toBe('text-warning')
  })
})
