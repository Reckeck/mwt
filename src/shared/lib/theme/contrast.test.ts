// check-ui-ignore raw-color: colour test vectors for the contrast maths
import { describe, expect, it } from 'vitest'
import { checkPairs, ColorParseError, contrastRatio, toHex } from './contrast'

describe('contrastRatio', () => {
  it('black on white is 21', () => {
    expect(contrastRatio('#000', '#fff')).toBeCloseTo(21, 5)
  })

  it('#777 on #fff is about 4.48', () => {
    expect(contrastRatio('#777777', '#ffffff')).toBeCloseTo(4.48, 2)
  })

  it('accepts rgb() and hsl() forms', () => {
    expect(contrastRatio('rgb(0, 0, 0)', 'rgb(255 255 255)')).toBeCloseTo(21, 5)
    expect(contrastRatio('hsl(0 0% 0%)', 'hsl(0, 0%, 100%)')).toBeCloseTo(21, 5)
  })

  it('reads bare hsl() saturation and lightness as percentages in space syntax', () => {
    expect(toHex('hsl(0 0 1)')).toBe('#030303')
    expect(toHex('hsl(0 50 50)')).toBe('#bf4040')
  })

  it('refuses bare numbers in legacy comma hsl()', () => {
    expect(() => toHex('hsl(0, 50, 50)')).toThrow(ColorParseError)
  })

  it('oklch red matches sRGB red (4.00 on white)', () => {
    expect(contrastRatio('oklch(0.62796 0.25768 29.234)', '#fff')).toBeCloseTo(contrastRatio('#ff0000', '#fff'), 1)
    expect(contrastRatio('oklch(0% 0 0)', 'oklch(100% 0 0)')).toBeCloseTo(21, 2)
  })

  it('composites #rrggbbaa foreground on the background', () => {
    // 50% black on white is about sRGB 127 grey
    expect(contrastRatio('#00000080', '#ffffff')).toBeCloseTo(contrastRatio('#7f7f7f', '#ffffff'), 1)
  })

  it('composites a translucent background on the surface', () => {
    expect(contrastRatio('#000', '#ffffff00', '#fff')).toBeCloseTo(21, 5)
    expect(contrastRatio('#fff', '#00000000', '#000')).toBeCloseTo(21, 5)
  })

  it('throws a typed error for an unknown colour', () => {
    expect(() => contrastRatio('banana', '#fff')).toThrow(ColorParseError)
    expect(() => contrastRatio('#12', '#fff')).toThrow(ColorParseError)
  })
})

describe('checkPairs', () => {
  it('returns only failing pairs with their ratio', () => {
    const tokens = { '--fg': '#777777', '--bg': '#ffffff', '--ink': '#000000' }
    const failing = checkPairs(tokens, [
      { foreground: '--fg', background: '--bg', min: 4.5 },
      { foreground: '--ink', background: '--bg', min: 4.5 },
    ])
    expect(failing).toHaveLength(1)
    expect(failing[0].foreground).toBe('--fg')
    expect(failing[0].ratio).toBeCloseTo(4.48, 2)
  })
})
