import { onScopeDispose } from 'vue'

/**
 * The app's keys in one place: who binds them, what Settings › Keys lists,
 * what a hint shows («Ctrl K»). A slice binds its key with `useHotkey`; the
 * list here is only the names and the combos.
 */
export type HotkeyId = 'palette' | 'quickAdd' | 'sidebar' | 'saveAndNext'

export interface Hotkey {
  id: HotkeyId
  /** `mod` is Ctrl, or ⌘ on a Mac. */
  combo: string
  /** i18n key of what it does. */
  key: string
}

export const HOTKEYS: Record<HotkeyId, Hotkey> = {
  palette: { id: 'palette', combo: 'mod+k', key: 'hotkey.palette' },
  quickAdd: { id: 'quickAdd', combo: 'mod+n', key: 'hotkey.quickAdd' },
  sidebar: { id: 'sidebar', combo: 'mod+b', key: 'hotkey.sidebar' },
  saveAndNext: { id: 'saveAndNext', combo: 'mod+enter', key: 'hotkey.saveAndNext' },
}

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

/** «Ctrl K» / «⌘ K», for a hint beside a button. */
export function hotkeyLabel(id: HotkeyId): string {
  return HOTKEYS[id].combo
    .split('+')
    .map((part) => (part === 'mod' ? (MAC ? '⌘' : 'Ctrl') : part === 'enter' ? 'Enter' : part.toUpperCase()))
    .join(' ')
}

/** Run `handler` on the key while the calling component lives. */
export function useHotkey(id: HotkeyId, handler: (event: KeyboardEvent) => void): void {
  const listener = (event: KeyboardEvent) => {
    if (event.defaultPrevented || !matchesHotkey(event, id)) return
    event.preventDefault()
    handler(event)
  }
  window.addEventListener('keydown', listener)
  onScopeDispose(() => window.removeEventListener('keydown', listener))
}
