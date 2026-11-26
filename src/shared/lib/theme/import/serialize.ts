/**
 * Export format of a theme (themes CE5, CI1): a header comment with name, mode and app version, then the
 * whole token whitelist in a single `:root { ... }` rule. Pure, no DOM. The output imports back unchanged.
 */
import { THEME_TOKENS } from '../tokens'
import type { Theme } from '../types'

/** Keeps a header value on one line and unable to close the comment. */
function headerValue(text: string): string {
  return text.replace(/\*\//g, '* /').replace(/\s+/g, ' ').trim()
}

export function serializeTheme(theme: Pick<Theme, 'name' | 'mode' | 'tokens'>, appVersion: string): string {
  const header = [
    `/* name: ${headerValue(theme.name)} */`,
    `/* mode: ${theme.mode} */`,
    `/* app version: ${headerValue(appVersion)} */`,
  ]
  const body = THEME_TOKENS.map((token) => `  ${token}: ${theme.tokens[token]};`)
  return `${header.join('\n')}\n:root {\n${body.join('\n')}\n}\n`
}
