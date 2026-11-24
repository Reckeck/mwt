import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

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
