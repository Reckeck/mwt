import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { applyTheme, clearTheme } from './apply'
import { defaultDark } from './themes/default-dark'
import { defaultLight } from './themes/default-light'
import { THEME_TOKENS } from './tokens'
import type { Theme } from './types'

const css = readFileSync(resolve(process.cwd(), 'src/app/styles.css'), 'utf8')

afterEach(() => {
  clearTheme(document.documentElement)
  document.documentElement.className = ''
  document.documentElement.removeAttribute('data-theme')
})

describe('applyTheme', () => {
  it('sets every whitelisted token inline and the mode markers', () => {
    const root = document.documentElement
    applyTheme(defaultDark, root)
    for (const token of THEME_TOKENS) {
      expect(root.style.getPropertyValue(token)).toBe(defaultDark.tokens[token])
    }
    expect(root.classList.contains('dark')).toBe(true)
    expect(root.getAttribute('data-theme')).toBe('dark')
    expect(root.style.colorScheme).toBe('dark')
    applyTheme(defaultLight, root)
    expect(root.classList.contains('dark')).toBe(false)
    expect(root.getAttribute('data-theme')).toBe('light')
    expect(root.style.colorScheme).toBe('light')
  })

  it('adds the theme-switching frame only when the mode changes', () => {
    const root = document.documentElement
    applyTheme(defaultLight, root)
    expect(root.classList.contains('theme-switching')).toBe(true)
    root.classList.remove('theme-switching')
    applyTheme(defaultLight, root)
    expect(root.classList.contains('theme-switching')).toBe(false)
  })

  it('ignores properties outside the whitelist and never touches --font-*', () => {
    const root = document.documentElement
    root.style.setProperty('--font-sans', 'Original')
    const theme = {
      ...defaultLight,
      tokens: { ...defaultLight.tokens, '--font-sans': 'Evil', '--font-mono': 'Evil', '--port-text': 'red' },
    } as unknown as Theme
    applyTheme(theme, root)
    expect(root.style.getPropertyValue('--font-sans')).toBe('Original')
    expect(root.style.getPropertyValue('--font-mono')).toBe('')
    expect(root.style.getPropertyValue('--port-text')).toBe('')
    root.style.removeProperty('--font-sans')
  })

  it('clearTheme removes the inline tokens', () => {
    const root = document.documentElement
    applyTheme(defaultLight, root)
    clearTheme(root)
    for (const token of THEME_TOKENS) expect(root.style.getPropertyValue(token)).toBe('')
  })
})

describe('radius derivation (TK4)', () => {
  function derived(name: string): string {
    return (css.match(new RegExp(`--radius-${name}:\\s*([^;]+);`))?.[1] ?? '').trim()
  }

  /** Evaluates `var(--radius)` or `calc(var(--radius) +/- Npx)` for a radius in px. */
  function evaluate(formula: string, radiusPx: number): number {
    if (formula === 'var(--radius)') return radiusPx
    const match = formula.match(/^calc\(var\(--radius\) ([+-]) (\d+)px\)$/)
    if (!match) throw new Error(`Unexpected formula ${formula}`)
    return radiusPx + (match[1] === '-' ? -1 : 1) * Number(match[2])
  }

  it('gives 8 / 12 / 16 / 24 px at the Default radius', () => {
    expect(defaultLight.tokens['--radius']).toBe('0.75rem')
    expect(defaultDark.tokens['--radius']).toBe('0.75rem')
    const radiusPx = Number.parseFloat(defaultLight.tokens['--radius']) * 16
    expect(['sm', 'md', 'lg', 'xl'].map((name) => evaluate(derived(name), radiusPx))).toEqual([8, 12, 16, 24])
  })

  it('follows a different theme radius', () => {
    expect(['sm', 'md', 'lg', 'xl'].map((name) => evaluate(derived(name), 0))).toEqual([-4, 0, 4, 12])
  })
})
