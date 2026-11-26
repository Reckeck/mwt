import { describe, expect, it } from 'vitest'

import { sourceValueText } from './source-value'

describe('sourceValueText', () => {
  it('prefers the number, then the last point, then the text', () => {
    expect(sourceValueText(null)).toBeNull()
    expect(sourceValueText({ number: '6.35', series: [], text: 'x' } as unknown as SourceValue)).toBe('6.35')
    expect(
      sourceValueText({ number: null, series: [{ value: '1' }, { value: '2' }], text: 'x' } as unknown as SourceValue),
    ).toBe('2')
    expect(sourceValueText({ number: null, series: [], text: 'x' } as unknown as SourceValue)).toBe('x')
  })
})
