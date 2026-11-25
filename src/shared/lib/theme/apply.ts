import { THEME_TOKENS } from './tokens'
import type { Theme } from './types'

const WHITELIST: ReadonlySet<string> = new Set(THEME_TOKENS)

/**
 * Applies a theme to `root` (TK6): only whitelisted tokens become inline custom properties; the
 * mode sets `.dark`, `data-theme` and `colorScheme`. A mode change gets one frame without
 * transitions, like the preference switch in `shared/lib/theme.ts`.
 */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement): void {
  const changed = root.getAttribute('data-theme') !== theme.mode
  if (changed) root.classList.add('theme-switching')
  for (const [name, value] of Object.entries(theme.tokens)) {
    if (WHITELIST.has(name)) root.style.setProperty(name, value)
  }
  root.classList.toggle('dark', theme.mode === 'dark')
  root.setAttribute('data-theme', theme.mode)
  root.style.colorScheme = theme.mode
  if (changed) requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove('theme-switching')))
}

/** Removes every inline token set by `applyTheme`; the stylesheet defaults show again. */
export function clearTheme(root: HTMLElement = document.documentElement): void {
  for (const name of THEME_TOKENS) root.style.removeProperty(name)
}
