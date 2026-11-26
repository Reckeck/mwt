// check-ui-ignore raw-color: theme fixtures are colour data fed to the parser, not UI styling
import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { applyTheme, clearTheme } from '../apply'
import { THEME_TOKENS } from '../tokens'
import type { Theme } from '../types'
import { parseThemeCss } from './parse'
import { validateTheme } from './validate'

const DIRECTORY = resolve(process.cwd(), 'src/shared/lib/theme/import/fixtures/hostile')
const MAX_PARSE_MS = 1000
const HOSTILE = /url\s*\(|expression\s*\(|javascript\s*:|-moz-binding|@import|<\/?style|<script|[<>{}]/i

const fixtures = readdirSync(DIRECTORY)
  .filter((file) => file.endsWith('.css'))
  .sort()
  .map((file) => ({ file, text: readFileSync(resolve(DIRECTORY, file), 'utf8') }))

afterEach(() => {
  clearTheme(document.documentElement)
})

describe('hostile corpus', () => {
  it('has at least 15 fixtures', () => {
    expect(fixtures.length).toBeGreaterThanOrEqual(15)
  })

  it.each(fixtures)('$file is rejected or sanitised within the time limit', ({ text }) => {
    const started = performance.now()
    const parsed = parseThemeCss(text)
    const result = validateTheme(text)
    expect(performance.now() - started).toBeLessThan(MAX_PARSE_MS)

    if (result.errors.length > 0) {
      expect(result.theme).toBeUndefined()
      return
    }
    expect(parsed.errors).toEqual([])
    const tokens = Object.entries(result.theme?.tokens ?? {})
    for (const [name, value] of tokens) {
      expect(THEME_TOKENS as readonly string[]).toContain(name)
      expect(value).not.toMatch(HOSTILE)
    }
  })

  it('applies CSS regenerated from the parsed map, whitelisted names only, never the raw text', () => {
    const text = ':root {\n  --background: #123456;\n  --evil-token: red;\n}\n'
    const result = validateTheme(text)
    expect(result.errors).toEqual([])
    const theme: Theme = {
      ...(result.theme as NonNullable<typeof result.theme>),
      id: 'hostile-test',
      kind: 'custom',
    } as unknown as Theme
    const root = document.createElement('div')
    const names: string[] = []
    const original = root.style.setProperty.bind(root.style)
    root.style.setProperty = (name: string, value: string | null, priority?: string) => {
      names.push(name)
      original(name, value, priority)
    }
    applyTheme(theme, root)
    expect(names.length).toBeGreaterThan(0)
    const allowed = [...THEME_TOKENS, 'color-scheme'] as string[]
    for (const name of names) expect(allowed).toContain(name)
    expect(names).not.toContain('--evil-token')
    expect(root.getAttribute('style')).not.toContain('evil')
    expect(root.style.getPropertyValue('--background')).toBe('#123456')
  })

  it.each(['constructor', '__proto__', 'toString', 'hasOwnProperty'])(
    'a prototype key %s as a colour is a per-line error, never a throw',
    (word) => {
      const text = readFileSync(resolve(DIRECTORY, `proto-${word}.css`), 'utf8')
      const result = validateTheme(text)
      expect(result.theme).toBeUndefined()
      expect(result.errors.length).toBeGreaterThan(0)
      expect(result.errors[0]).toMatchObject({ line: 2, col: expect.any(Number) })
    },
  )
})
