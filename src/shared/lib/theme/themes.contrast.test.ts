import { describe, expect, it } from 'vitest'
import { checkPairs } from './contrast'
import { CONTRAST_PAIRS } from './contrast-pairs'
import { builtinThemes } from './registry'

/**
 * Spec Q15 / TC3: the Default themes keep today's colours, so these four chart pairs stay below 3:1
 * until the chart colours are tuned in the redesign (ui-review T71). Nothing else may be listed here.
 * Each entry is asserted to still fail, so the list cannot go stale.
 */
const CHART_EXCEPTIONS: readonly { themeId: string; token: string }[] = [
  { themeId: 'default-light', token: '--chart-4' },
  { themeId: 'default-light', token: '--chart-5' },
  { themeId: 'default-light', token: '--chart-crypto' },
  { themeId: 'default-dark', token: '--chart-1' },
]

const isExempt = (themeId: string, token: string) =>
  CHART_EXCEPTIONS.some((entry) => entry.themeId === themeId && entry.token === token)

describe('built-in themes meet contrast (TC1-TC3)', () => {
  it.each(builtinThemes.map((theme) => [theme.id, theme] as const))('%s', (_id, theme) => {
    const failing = checkPairs(theme.tokens, CONTRAST_PAIRS)
      .filter((pair) => !isExempt(theme.id, pair.foreground))
      .map(
        (pair) =>
          `${theme.id}: ${pair.foreground} on ${pair.background} is ${pair.ratio.toFixed(2)}:1, needs ${pair.min}:1`,
      )
    expect(failing).toEqual([])
  })

  it.each(CHART_EXCEPTIONS.map((entry) => [`${entry.themeId} ${entry.token}`, entry] as const))(
    'exception %s is still below 3:1',
    (_name, entry) => {
      const theme = builtinThemes.find((candidate) => candidate.id === entry.themeId)
      expect(theme, `unknown theme ${entry.themeId}`).toBeDefined()
      const failing = checkPairs(theme?.tokens ?? {}, CONTRAST_PAIRS)
      expect(failing.some((pair) => pair.foreground === entry.token)).toBe(true)
    },
  )
})
