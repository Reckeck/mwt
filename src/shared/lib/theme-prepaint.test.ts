import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { PREPAINT_TABLE } from '@/shared/lib/theme/prepaint-table'
import { builtinThemes } from '@/shared/lib/theme/registry'

const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8')
const css = readFileSync(resolve(process.cwd(), 'src/app/styles.css'), 'utf8')

function background(block: string): string {
  const start = css.indexOf(`\n${block} {`)
  const match = css.slice(start).match(/--background:\s*(#[0-9a-fA-F]{6});/)
  return (match?.[1] ?? '').toLowerCase()
}

describe('pre-paint canvas colour', () => {
  it('paints the light canvas of the tokens before the stylesheet arrives', () => {
    expect(html).toMatch(new RegExp(`html \\{ background: ${background(':root')};`, 'i'))
  })

  it('paints the dark canvas of the tokens for html.dark', () => {
    expect(html).toMatch(new RegExp(`html\\.dark \\{ background: ${background('.dark')};`, 'i'))
  })

  it('still sets the stored theme class and data-theme in the inline script', () => {
    expect(html).toContain("localStorage.getItem('trade-app:theme')")
    expect(html).toContain("root.setAttribute('data-theme'")
  })
})

function inlineScript(): string {
  const match = html.match(/<script>([\s\S]*?)<\/script>/)
  return match?.[1] ?? ''
}

function htmlTable(): Record<string, [string, string]> {
  const match = html.match(/const TABLE = (\{.*\})/)
  return JSON.parse(match?.[1] ?? '{}')
}

describe('pre-paint table (TP5)', () => {
  it('has an entry per built-in theme equal to its --background and mode', () => {
    expect(Object.keys(PREPAINT_TABLE).sort()).toEqual(builtinThemes.map((theme) => theme.id).sort())
    for (const theme of builtinThemes) {
      expect(PREPAINT_TABLE[theme.id]).toEqual([theme.tokens['--background'].toLowerCase(), theme.mode])
    }
  })

  it('is the same table the inline script of index.html carries', () => {
    expect(htmlTable()).toEqual(PREPAINT_TABLE)
  })
})

describe('pre-paint inline script', () => {
  const root = document.documentElement

  function run(stored: Record<string, string>, systemDark: boolean): void {
    localStorage.clear()
    for (const [key, value] of Object.entries(stored)) localStorage.setItem(`trade-app:${key}`, value)
    vi.stubGlobal('matchMedia', () => ({ matches: systemDark }))
    new Function(inlineScript())()
  }

  beforeEach(() => {
    root.className = ''
    root.removeAttribute('data-theme')
    root.removeAttribute('style')
    for (const node of document.head.querySelectorAll('style[data-prepaint]')) node.remove()
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    localStorage.clear()
  })

  function canvasRule(): string {
    return [...document.head.querySelectorAll('style[data-prepaint]')].at(-1)?.textContent ?? ''
  }

  it('Light uses theme-id and ignores the system mode', () => {
    run({ theme: 'light', 'theme-id': 'alucard' }, true)
    expect(root.classList.contains('dark')).toBe(false)
    expect(root.getAttribute('data-theme')).toBe('light')
    expect(canvasRule()).toContain(`background: ${PREPAINT_TABLE.alucard[0]};`)
  })

  it('Dark uses theme-id', () => {
    run({ theme: 'dark', 'theme-id': 'dracula' }, false)
    expect(root.classList.contains('dark')).toBe(true)
    expect(root.style.colorScheme).toBe('dark')
    expect(canvasRule()).toContain(`background: ${PREPAINT_TABLE.dracula[0]};`)
  })

  it('Auto picks the light slot under a light system and the dark slot under a dark one', () => {
    const stored = { theme: 'auto', 'theme-light-id': 'alucard', 'theme-dark-id': 'dracula' }
    run(stored, false)
    expect(root.getAttribute('data-theme')).toBe('light')
    expect(canvasRule()).toContain(PREPAINT_TABLE.alucard[0])
    root.className = ''
    run(stored, true)
    expect(root.getAttribute('data-theme')).toBe('dark')
    expect(root.classList.contains('dark')).toBe(true)
    expect(canvasRule()).toContain(PREPAINT_TABLE.dracula[0])
  })

  it('falls back to Default of the mode for an unknown id or an id of the other mode', () => {
    run({ theme: 'dark', 'theme-id': 'removed-theme' }, false)
    expect(canvasRule()).toContain(PREPAINT_TABLE['default-dark'][0])
    run({ theme: 'dark', 'theme-id': 'alucard' }, false)
    expect(canvasRule()).toContain(PREPAINT_TABLE['default-dark'][0])
  })

  it('uses the mirrored custom background and foreground before the table', () => {
    run({ theme: 'dark', 'theme-id': 'custom', 'theme-background': '#102030', 'theme-foreground': '#ddeeff' }, false)
    expect(canvasRule()).toContain('background: #102030;')
    expect(canvasRule()).toContain('color: #ddeeff;')
  })

  it('ignores a mirrored value that is not a #hex (CF3)', () => {
    run(
      {
        theme: 'dark',
        'theme-id': 'dracula',
        'theme-background': 'red; } body { display:none',
        'theme-foreground': 'url(x)',
      },
      false,
    )
    expect(canvasRule()).toContain(`background: ${PREPAINT_TABLE.dracula[0]};`)
    expect(canvasRule()).not.toContain('color: url')
    expect(canvasRule()).not.toContain('display')
  })
})
