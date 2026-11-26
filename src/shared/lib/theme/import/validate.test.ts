import { describe, expect, it } from 'vitest'
import { defaultDark } from '../themes/default-dark'
import { defaultLight } from '../themes/default-light'
import { THEME_TOKENS } from '../tokens'
import { MAX_DECLARATIONS, MAX_VALUE_LENGTH, validateTheme } from './validate'

function messages(text: string): string[] {
  return validateTheme(text).errors.map((issue) => `${issue.line}:${issue.col} ${issue.message}`)
}

function fullFile(header = '', overrides: Record<string, string> = {}): string {
  const tokens = { ...defaultDark.tokens, ...overrides }
  const body = Object.entries(tokens)
    .map(([name, value]) => `  ${name}: ${value};`)
    .join('\n')
  return `${header}\n:root {\n${body}\n}\n`
}

describe('validateTheme', () => {
  it('accepts a complete file with name and mode', () => {
    const result = validateTheme(fullFile('/* name: Mine */\n/* mode: dark */'))
    expect(result.errors).toEqual([])
    expect(result.warnings).toEqual([])
    expect(result.contrast).toEqual([])
    expect(result.theme).toMatchObject({ name: 'Mine', mode: 'dark', modeGuessed: false })
    expect(result.theme?.tokens['--background']).toBe(defaultDark.tokens['--background'])
  })

  it('warns about an unknown token with its line and drops it', () => {
    const result = validateTheme(':root {\n  --background: #fff;\n  --font-sans: Comic Sans;\n}')
    expect(result.errors).toEqual([])
    expect(result.warnings[0]).toMatchObject({ line: 3, col: 3 })
    expect(result.warnings[0].message).toContain('--font-sans')
    expect(result.theme && '--font-sans' in result.theme.tokens).toBe(false)
  })

  it('rejects font-family as a non-custom property', () => {
    expect(messages(':root {\n  font-family: serif;\n}')).toEqual([
      "2:3 property 'font-family' is not a custom property",
    ])
  })

  it('falls back to the base theme of the declared mode with one warning', () => {
    const light = validateTheme('/* mode: light */\n:root { --background: #fafafa; }')
    expect(light.theme?.tokens['--foreground']).toBe(defaultLight.tokens['--foreground'])
    expect(light.theme?.tokens['--background']).toBe('#fafafa')
    const missing = light.warnings.filter((warning) => warning.message.startsWith('missing tokens'))
    expect(missing).toHaveLength(1)
    expect(missing[0].message).toContain('--foreground')
    expect(missing[0].message).not.toContain('--background,')
    const dark = validateTheme('/* mode: dark */\n:root { --background: #fafafa; }')
    expect(dark.theme?.tokens['--foreground']).toBe(defaultDark.tokens['--foreground'])
  })

  it('reads the name and falls back to the file name', () => {
    expect(validateTheme('/* name:  Nice one */\n:root { --background: #000; }').theme?.name).toBe('Nice one')
    expect(validateTheme(':root { --background: #000; }', 'C:\\x\\my-theme.css').theme?.name).toBe('my-theme')
    expect(validateTheme(':root { --background: #000; }').theme?.name).toBe('Custom theme')
  })

  it('guesses the mode from the background and flags it', () => {
    expect(validateTheme(':root { --background: #ffffff; }').theme).toMatchObject({ mode: 'light', modeGuessed: true })
    expect(validateTheme(':root { --background: oklch(0.2 0 0); }').theme).toMatchObject({
      mode: 'dark',
      modeGuessed: true,
    })
  })

  it('ignores an invalid mode comment with a warning', () => {
    const result = validateTheme('/* mode: sepia */\n:root { --background: #fff; }')
    expect(result.theme?.modeGuessed).toBe(true)
    expect(result.warnings.some((warning) => warning.message.includes('sepia'))).toBe(true)
  })

  it.each([
    '#abc',
    '#abcd',
    '#aabbcc',
    '#aabbccdd',
    'rgb(1 2 3)',
    'rgba(1, 2, 3, 0.5)',
    'rgb(1 2 3 / 50%)',
    'hsl(120 50% 50%)',
    'hsla(120, 50%, 50%, 0.3)',
    'oklch(0.5 0.1 200)',
    'oklch(0.5 0.1 200deg / 0.4)',
    'oklab(0.5 0.1 -0.1)',
    'rebeccapurple',
    'Transparent',
  ])('accepts colour %s', (value) => {
    expect(messages(`:root { --primary: ${value}; }`)).toEqual([])
  })

  it.each(['#abcde', 'rgb(1 2)', 'rgb(a b c)', 'currentColor', 'notacolour', '12px', 'oklch(1 0 0 0 0)'])(
    'rejects colour %s',
    (value) => {
      expect(messages(`:root { --primary: ${value}; }`)).toHaveLength(1)
    },
  )

  it.each(['0', '0px', '8px', '0.5rem', '2rem', '32px'])('accepts radius %s', (value) => {
    expect(messages(`:root { --radius: ${value}; }`)).toEqual([])
  })

  it.each(['2.1rem', '33px', '-1px', '5', 'red', '1em'])('rejects radius %s', (value) => {
    expect(messages(`:root { --radius: ${value}; }`)).toHaveLength(1)
  })

  it.each(['var(--x)', 'calc(1px + 2px)', 'env(safe-area-inset-top)', 'rgb(var(--x) 0 0)'])('rejects %s', (value) => {
    expect(messages(`:root { --radius: ${value}; }`)[0]).toMatch(/not allowed/)
    expect(messages(`:root { --unknown: ${value}; }`)[0]).toMatch(/not allowed/)
  })

  it('rejects a file over 64 KiB', () => {
    const text = `/* ${'a'.repeat(64 * 1024)} */\n:root { --background: #fff; }`
    expect(validateTheme(text).errors[0].message).toContain('64 KiB')
    expect(validateTheme(text).theme).toBeUndefined()
  })

  it('rejects more than 256 declarations', () => {
    const body = Array.from({ length: MAX_DECLARATIONS + 1 }, (_, index) => `--x${index}: red;`).join('\n')
    expect(validateTheme(`:root {\n${body}\n}`).errors[0].message).toContain('256 declarations')
    const ok = Array.from({ length: MAX_DECLARATIONS }, (_, index) => `--x${index}: red;`).join('\n')
    expect(validateTheme(`:root {\n${ok}\n}`).errors).toEqual([])
  })

  it('rejects a value over 512 characters with its line', () => {
    const value = `${'a'.repeat(MAX_VALUE_LENGTH + 1)}`
    expect(messages(`:root {\n  --unknown: ${value};\n}`)).toEqual([
      `2:3 value of --unknown is longer than ${MAX_VALUE_LENGTH} characters`,
    ])
  })

  it('returns no theme when there is a parse error', () => {
    const result = validateTheme('.a { --background: #fff; }')
    expect(result.errors).toHaveLength(1)
    expect(result.theme).toBeUndefined()
  })

  it('knows every whitelisted token', () => {
    const body = THEME_TOKENS.map((token) => `${token}: ${token === '--radius' ? '4px' : '#123456'};`).join('\n')
    const result = validateTheme(`/* mode: dark */\n:root { ${body} }`)
    expect(result.errors).toEqual([])
    expect(result.warnings).toEqual([])
  })
})
