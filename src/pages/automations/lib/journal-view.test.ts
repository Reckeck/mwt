import { describe, expect, it } from 'vitest'

import { journalTone } from './journal-view'

describe('journalTone', () => {
  it('colours the outcomes', () => {
    expect(journalTone('done')).toBe('text-success')
    expect(journalTone('asking')).toBe('text-warning')
    expect(journalTone('failed')).toBe('text-destructive')
    expect(journalTone('shown')).toBe('text-muted-foreground')
  })
})
