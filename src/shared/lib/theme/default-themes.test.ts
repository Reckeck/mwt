import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { contrastRatio } from './contrast'
import { defaultDark } from './themes/default-dark'
import { defaultLight } from './themes/default-light'
import { THEME_TOKENS } from './tokens'
import type { Theme } from './types'

const css = readFileSync(resolve(process.cwd(), 'src/app/styles.css'), 'utf8')

function readBlock(selector: string): Map<string, string> {
  const start = css.indexOf(`\n${selector} {`)
  const end = css.indexOf('\n}\n', start)
  const variables = new Map<string, string>()
  for (const match of css.slice(start, end).matchAll(/^\s+(--[\w-]+):\s*([^;]+);/gm)) {
    variables.set(match[1] as string, (match[2] as string).trim())
  }
  return variables
}

function resolveValue(variables: Map<string, string>, value: string): string {
  const reference = value.match(/^var\((--[\w-]+)\)$/)
  return reference ? resolveValue(variables, variables.get(reference[1] as string) as string) : value
}

// --radius is not a CSS variable yet (TK4 introduces it), so it is not compared.
const comparedTokens = THEME_TOKENS.filter((token) => token !== '--radius')

describe.each([
  [':root', defaultLight],
  ['.dark', defaultDark],
] as [string, Theme][])('%s equals the Default theme (TB3)', (selector, theme) => {
  const variables = readBlock(selector)
  it.each(comparedTokens)('%s', (token) => {
    expect(theme.tokens[token]).toBe(resolveValue(variables, variables.get(token) as string))
  })
})

describe('money tokens (TK3)', () => {
  it.each([
    [':root', defaultLight],
    ['.dark', defaultDark],
  ] as [string, Theme][])('%s links to the semantic colours and keeps transfer legible', (selector, theme) => {
    const variables = readBlock(selector)
    expect(variables.get('--money-income')).toBe('var(--success)')
    expect(variables.get('--money-expense')).toBe('var(--destructive)')
    expect(variables.get('--money-in-transit')).toBe('var(--warning)')
    expect(contrastRatio(theme.tokens['--money-transfer'], theme.tokens['--background'])).toBeGreaterThanOrEqual(4.5)
    expect(contrastRatio(theme.tokens['--money-transfer'], theme.tokens['--card'])).toBeGreaterThanOrEqual(4.5)
  })
})
