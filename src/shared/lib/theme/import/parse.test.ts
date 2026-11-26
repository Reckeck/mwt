import { describe, expect, it } from 'vitest'
import { formatParseError, parseThemeCss } from './parse'

function errorsOf(source: string): string[] {
  const result = parseThemeCss(source)
  expect(result.declarations).toEqual([])
  return result.errors.map(formatParseError)
}

describe('parseThemeCss', () => {
  it('parses a valid file with positions', () => {
    const result = parseThemeCss('/* theme */\n:root {\n  --background: #fff;\n  --foreground: oklch(0.2 0 0);\n}\n')
    expect(result.errors).toEqual([])
    expect(result.declarations).toEqual([
      { name: '--background', value: '#fff', line: 3, col: 3 },
      { name: '--foreground', value: 'oklch(0.2 0 0)', line: 4, col: 3 },
    ])
  })

  it('accepts a last declaration without a semicolon', () => {
    expect(parseThemeCss(':root{--a:red;--b:blue}').declarations).toHaveLength(2)
  })

  it('rejects another selector', () => {
    expect(errorsOf('\n.dark { --a: red; }')).toEqual(["2:1 selector '.dark' is not allowed, only :root"])
  })

  it('rejects a second rule', () => {
    expect(errorsOf(':root { --a: red; }\n\n:root { --b: blue; }')).toEqual(['3:1 only one rule is allowed'])
  })

  it.each(['@import', '@font-face', '@media'])('rejects at-rule %s', (rule) => {
    expect(errorsOf(`:root { --a: red; }\n${rule} x;`)).toEqual([`2:1 at-rule ${rule} is not allowed`])
  })

  it('rejects an at-rule before the rule', () => {
    expect(errorsOf('@import "x.css";\n:root { --a: red; }')).toEqual(['1:1 at-rule @import is not allowed'])
  })

  it.each([
    ['url(x)', 'url('],
    ['image-set(x 1x)', 'image-set('],
    ['src(x)', 'src('],
    ['expression(1)', 'expression('],
    ['javascript:alert(1)', 'javascript:'],
  ])('rejects %s', (value, found) => {
    expect(errorsOf(`:root {\n  --a: red;\n  --b: ${value};\n}`)).toEqual([`3:8 '${found}' is not allowed in --b`])
  })

  it('decodes CSS escapes before the checks', () => {
    expect(errorsOf(':root {\n  --a: \\75 rl(x);\n}')).toEqual(["2:8 'url(' is not allowed in --a"])
    expect(errorsOf(':root {\n  --a: \\000075rl(x);\n}')).toEqual(["2:8 'url(' is not allowed in --a"])
    expect(errorsOf(':root {\n  --a: u\\rl(x);\n}')).toEqual(["2:8 'url(' is not allowed in --a"])
    expect(errorsOf('\\3a root { --a: red; }\n:root { --b: red; }')).toEqual(['2:1 only one rule is allowed'])
  })

  it('does not let a comment hide a forbidden value', () => {
    expect(errorsOf(':root {\n  --a: /* x */ url(y);\n}')).toEqual(["2:16 'url(' is not allowed in --a"])
  })

  it('rejects a malformed block', () => {
    expect(errorsOf(':root {\n  --a: red;\n')).toEqual(["1:7 malformed block: missing '}'"])
    expect(errorsOf(':root {\n  --a: red\n')).toEqual(["1:7 malformed block: missing '}'"])
    expect(errorsOf(':root {\n  --a: red;\n  .x { --b: red; }\n}')).toEqual([
      '3:6 malformed block: nested block is not allowed',
    ])
    expect(errorsOf(':root {\n  --a red;\n}')).toEqual(["2:3 malformed declaration '--a red': expected ':'"])
    expect(errorsOf('--a: red;')).toEqual(["1:1 malformed block: expected '{'"])
    expect(errorsOf(':root {\n  --a: (red;\n}')).toEqual(['2:8 unbalanced parentheses in --a'])
    expect(errorsOf('/* open\n:root { --a: red; }')).toEqual(['1:1 unterminated comment'])
  })

  it('rejects a non-custom-property declaration', () => {
    expect(errorsOf(':root {\n  --a: red;\n  color: red;\n}')).toEqual([
      "3:3 property 'color' is not a custom property",
    ])
    expect(errorsOf(':root {\n  font-family: x;\n}')).toEqual(["2:3 property 'font-family' is not a custom property"])
  })

  it('rejects hostile characters in values', () => {
    expect(errorsOf(':root {\n  --a: </style>;\n}')).toEqual(['2:8 forbidden character in value in --a'])
    expect(errorsOf(':root {\n  --a: red\\;;\n}')).toEqual(['2:8 forbidden character in value in --a'])
  })

  it('reports an empty file', () => {
    expect(errorsOf('  \n')).toEqual(['1:1 expected a :root block'])
  })

  it('returns nothing applicable when any error exists, even after valid declarations', () => {
    const result = parseThemeCss(':root { --a: red; color: red; --b: blue; }')
    expect(result.errors).toHaveLength(1)
    expect(result.declarations).toEqual([])
  })

  it('collects several declaration errors', () => {
    expect(errorsOf(':root {\n  color: red;\n  --a: url(x);\n}')).toHaveLength(2)
  })

  it('stays linear on many colon-less declarations', () => {
    const body = 'a;'.repeat(100000)
    const started = performance.now()
    const result = parseThemeCss(`:root{${body}}`)
    expect(performance.now() - started).toBeLessThan(2000)
    expect(result.declarations).toEqual([])
    expect(result.errors.length).toBeGreaterThan(0)
  })
})
