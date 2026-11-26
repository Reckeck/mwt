// check-ui-ignore raw-color: theme fixtures are colour data fed to the parser, not UI styling
import { describe, expect, it } from 'vitest'
import { builtinThemes } from '../registry'
import { THEME_TOKENS } from '../tokens'
import { parseThemeCss } from './parse'
import { serializeTheme } from './serialize'
import { validateTheme } from './validate'

describe('serializeTheme', () => {
  it('writes a header and the whole whitelist in one :root rule', () => {
    const text = serializeTheme(builtinThemes[0], '1.2.3')
    expect(text.startsWith('/* name: ')).toBe(true)
    expect(text).toContain(`/* mode: ${builtinThemes[0].mode} */`)
    expect(text).toContain('1.2.3')
    expect(text.match(/:root\s*\{/g)).toHaveLength(1)
    const parsed = parseThemeCss(text)
    expect(parsed.errors).toEqual([])
    expect(parsed.declarations.map((declaration) => declaration.name)).toEqual([...THEME_TOKENS])
  })

  it('cannot be broken out of the header comment by the name', () => {
    const text = serializeTheme({ ...builtinThemes[0], name: 'a */ :root { } b\nc' }, '1')
    const result = validateTheme(text)
    expect(result.errors).toEqual([])
    expect(result.theme?.name).not.toContain('\n')
  })

  it.each(builtinThemes.map((theme) => [theme.id, theme] as const))('round-trips %s', (_id, theme) => {
    const result = validateTheme(serializeTheme(theme, '0.1.0'))
    expect(result.errors).toEqual([])
    expect(result.warnings).toEqual([])
    expect(result.theme?.mode).toBe(theme.mode)
    expect(result.theme?.modeGuessed).toBe(false)
    expect(result.theme?.name).toBe(theme.name)
    expect(result.theme?.tokens).toEqual(theme.tokens)
  })
})
