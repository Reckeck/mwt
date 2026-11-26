import { describe, expect, it } from 'vitest'
import { builtinThemes, getTheme } from './registry'

/** `../_notes/theme-list.md` in list order. Default is 0.75rem (not the list's 0.625): that is today's 12px --radius-md (TK4). */
const EXPECTED: readonly (readonly [id: string, mode: 'light' | 'dark', radius: string])[] = [
  ['default-light', 'light', '0.75rem'],
  ['default-dark', 'dark', '0.75rem'],
  ['dracula', 'dark', '0.5rem'],
  ['alucard', 'light', '0.5rem'],
  ['nord', 'dark', '0.375rem'],
  ['nord-snow-storm', 'light', '0.375rem'],
  ['solarized-dark', 'dark', '0.25rem'],
  ['solarized-light', 'light', '0.25rem'],
  ['gruvbox-dark', 'dark', '0.25rem'],
  ['gruvbox-light', 'light', '0.25rem'],
  ['catppuccin-mocha', 'dark', '0.75rem'],
  ['catppuccin-macchiato', 'dark', '0.75rem'],
  ['catppuccin-frappe', 'dark', '0.75rem'],
  ['catppuccin-latte', 'light', '0.75rem'],
  ['tokyo-night', 'dark', '0.5rem'],
  ['tokyo-night-storm', 'dark', '0.5rem'],
  ['tokyo-night-day', 'light', '0.5rem'],
  ['one-dark', 'dark', '0.375rem'],
  ['one-light', 'light', '0.375rem'],
  ['rose-pine', 'dark', '0.75rem'],
  ['rose-pine-moon', 'dark', '0.75rem'],
  ['rose-pine-dawn', 'light', '0.75rem'],
  ['everforest-dark', 'dark', '0.5rem'],
  ['everforest-light', 'light', '0.5rem'],
  ['kanagawa-wave', 'dark', '0.25rem'],
  ['kanagawa-lotus', 'light', '0.25rem'],
  ['github-dark', 'dark', '0.375rem'],
  ['github-light', 'light', '0.375rem'],
  ['ayu-mirage', 'dark', '0.5rem'],
  ['ayu-light', 'light', '0.5rem'],
]

describe('theme registry (TB1)', () => {
  it('ships exactly 30 themes with unique kebab-case ids', () => {
    expect(builtinThemes).toHaveLength(30)
    const ids = builtinThemes.map((theme) => theme.id)
    expect(new Set(ids).size).toBe(30)
    for (const id of ids) expect(id).toMatch(/^[a-z0-9]+(-[a-z0-9]+)*$/)
  })

  it('has 13 light and 17 dark themes (as counted in the approved list, not 15/15)', () => {
    expect(builtinThemes.filter((theme) => theme.mode === 'light')).toHaveLength(13)
    expect(builtinThemes.filter((theme) => theme.mode === 'dark')).toHaveLength(17)
  })

  it('matches the approved list in order: id, mode, radius', () => {
    expect(builtinThemes.map((theme) => [theme.id, theme.mode, theme.tokens['--radius']])).toEqual(EXPECTED)
  })

  it('getTheme finds by id and returns undefined for an unknown id', () => {
    expect(getTheme('dracula')?.name).toBe('Dracula')
    expect(getTheme('nope')).toBeUndefined()
  })
})
