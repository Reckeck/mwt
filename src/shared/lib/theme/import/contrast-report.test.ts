// check-ui-ignore raw-color: theme fixtures are colour data fed to the parser, not UI styling
import { describe, expect, it } from 'vitest'
import { builtinThemes } from '../registry'
import { reportContrast } from './contrast-report'

const base = () => ({ ...builtinThemes.find((theme) => theme.id === 'alucard')?.tokens }) as Record<string, string>

describe('reportContrast', () => {
  it('passes a built-in theme', () => {
    const report = reportContrast(base())
    expect(report).toEqual({ errors: [], warnings: [], contrast: [], contrastWarning: false })
  })

  it('warns with the pair named when muted text is at about 2:1', () => {
    const report = reportContrast({ ...base(), '--muted-foreground': '#8a8a8a', '--background': '#ffffff' })
    expect(report.errors).toEqual([])
    expect(report.contrastWarning).toBe(true)
    expect(report.warnings.some((warning) => warning.message.includes('--muted-foreground on --background'))).toBe(true)
  })

  it('errors when foreground on background is below 3:1', () => {
    const report = reportContrast({ ...base(), '--background': '#ffffff', '--foreground': '#bbbbbb' })
    expect(report.errors).toHaveLength(1)
    expect(report.errors[0].message).toContain('--foreground on --background')
    expect(report.contrast.some((pair) => pair.ratio < 3)).toBe(true)
  })

  it('warns, not errors, between 3:1 and 4.5:1', () => {
    const report = reportContrast({ ...base(), '--background': '#ffffff', '--foreground': '#8c8c8c' })
    expect(report.errors).toEqual([])
    expect(report.contrastWarning).toBe(true)
  })

  it('never throws on unreadable or missing colours', () => {
    expect(() => reportContrast({ '--foreground': 'nonsense' })).not.toThrow()
    expect(() => reportContrast({})).not.toThrow()
  })

  it.each([
    ['named', 'white', 'whitesmoke'],
    ['oklab', 'oklab(0.99 0 0)', 'oklab(0.97 0 0)'],
    ['turn unit', 'oklch(0.99 0 0.5turn)', 'oklch(0.97 0 0.25turn)'],
  ])('blocks a %s theme at about 1:1 instead of skipping it', (_label, background, foreground) => {
    const report = reportContrast({ ...base(), '--background': background, '--foreground': foreground })
    expect(report.errors).toHaveLength(1)
    expect(report.errors[0].message).toContain('--foreground on --background')
  })

  it('reports an unreadable colour as a blocking error for foreground on background', () => {
    const report = reportContrast({ ...base(), '--foreground': 'not-a-colour' })
    expect(report.errors.some((error) => error.message.includes('--foreground on --background'))).toBe(true)
  })

  it('reports an unreadable colour as a warning for other pairs', () => {
    const report = reportContrast({ ...base(), '--muted-foreground': 'not-a-colour' })
    expect(report.errors).toEqual([])
    expect(report.contrastWarning).toBe(true)
  })
})
