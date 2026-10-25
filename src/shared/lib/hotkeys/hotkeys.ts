import { onScopeDispose } from 'vue'

/**
 * The app's keys in one place: who binds them, what Settings › Keys lists,
 * what a hint shows («Ctrl K»). A slice binds its key with `useHotkey`; the
 * list here is only the names and the combos.
 */
export type HotkeyId = 'palette' | 'quickAdd' | 'sidebar' | 'saveAndNext' | SectionHotkey

/** Alt 1…5: a main section without the search (Paths 1 · 7). */
export type SectionHotkey = 'section1' | 'section2' | 'section3' | 'section4' | 'section5'
export const SECTION_HOTKEYS: SectionHotkey[] = ['section1', 'section2', 'section3', 'section4', 'section5']

export interface Hotkey {
  id: HotkeyId
  /** `mod` is Ctrl, or ⌘ on a Mac. */
  combo: string
  /** i18n key of what it does. */
  key: string
  /** Listed in Settings › Keys, in the order of the object below. */
  listed?: boolean
  /** Exists only where there is a sidebar (not on a phone). */
  desktopOnly?: boolean
}

export const HOTKEYS: Record<HotkeyId, Hotkey> = {
  quickAdd: { id: 'quickAdd', combo: 'mod+n', key: 'hotkey.quickAdd', listed: true },
  palette: { id: 'palette', combo: 'mod+k', key: 'hotkey.palette', listed: true },
  saveAndNext: { id: 'saveAndNext', combo: 'mod+enter', key: 'hotkey.saveAndNext', listed: true },
  sidebar: { id: 'sidebar', combo: 'mod+b', key: 'hotkey.sidebar', listed: true, desktopOnly: true },
  section1: { id: 'section1', combo: 'alt+1', key: 'hotkey.section1' },
  section2: { id: 'section2', combo: 'alt+2', key: 'hotkey.section2' },
  section3: { id: 'section3', combo: 'alt+3', key: 'hotkey.section3' },
  section4: { id: 'section4', combo: 'alt+4', key: 'hotkey.section4' },
  section5: { id: 'section5', combo: 'alt+5', key: 'hotkey.section5' },
}

/** The keys Settings › Keys lists, in the registry's order. */
export const LISTED_HOTKEYS: Hotkey[] = Object.values(HOTKEYS).filter((hotkey) => hotkey.listed)

const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform)

/** The key by its physical place, so a Russian layout still opens Ctrl K. */
function codeOf(key: string): string {
  if (key.length === 1 && /[a-z]/.test(key)) return `Key${key.toUpperCase()}`
  if (key.length === 1 && /[0-9]/.test(key)) return `Digit${key}`
  return key.charAt(0).toUpperCase() + key.slice(1)
}

export function matchesHotkey(event: KeyboardEvent, id: HotkeyId): boolean {
  const parts = HOTKEYS[id].combo.split('+')
  const key = parts.pop() ?? ''
  const mod = parts.includes('mod')
  if (mod !== (event.ctrlKey || event.metaKey)) return false
  if (parts.includes('shift') !== event.shiftKey || parts.includes('alt') !== event.altKey) return false
  return event.code === codeOf(key) || event.key.toLowerCase() === key
}

const KEY_LABELS: Record<string, string> = {
  mod: MAC ? '⌘' : 'Ctrl',
  alt: MAC ? '⌥' : 'Alt',
  shift: 'Shift',
  enter: 'Enter',
}

/** «Ctrl K» / «⌘ K», for a hint beside a button. */
export function hotkeyLabel(id: HotkeyId): string {
  return HOTKEYS[id].combo
    .split('+')
    .map((part) => KEY_LABELS[part] ?? part.toUpperCase())
    .join(' ')
}

/** A modal layer (a dialog, the settings window) is open: it holds the focus and the page is not clickable. */
const modalOpen = () => document.querySelector('[role="dialog"][data-state="open"]') !== null

/**
 * Run `handler` on the key while the calling component lives. The app's keys go to the page: while a
 * modal layer is open they wait (a palette or a second dialog over it would be out of reach).
 */
export function useHotkey(id: HotkeyId, handler: (event: KeyboardEvent) => void): void {
  const listener = (event: KeyboardEvent) => {
    if (event.defaultPrevented || !matchesHotkey(event, id) || modalOpen()) return
    event.preventDefault()
    handler(event)
  }
  window.addEventListener('keydown', listener)
  onScopeDispose(() => window.removeEventListener('keydown', listener))
}
