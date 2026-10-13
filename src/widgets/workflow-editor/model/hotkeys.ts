/** Keyboard shortcuts of the editor, for the help card and the handler. */
export const EDITOR_HOTKEYS = [
  { keys: 'Ctrl+Space / Shift+A', action: 'quickAdd' },
  { keys: 'Space', action: 'pan' },
  { keys: 'Ctrl+S', action: 'save' },
  { keys: 'Ctrl+Enter', action: 'emulate' },
  { keys: 'F1', action: 'help' },
  { keys: 'Delete', action: 'delete' },
  { keys: 'Ctrl+D', action: 'duplicate' },
  { keys: 'Ctrl+C / Ctrl+V', action: 'copyPaste' },
  { keys: 'Ctrl+Z / Ctrl+Shift+Z', action: 'undoRedo' },
  { keys: 'Ctrl+E', action: 'toggle' },
  { keys: 'Ctrl+A', action: 'selectAll' },
  { keys: 'F', action: 'fit' },
] as const

/** Whether the keystroke is typing into a field (then most shortcuts stay off). */
export function isTyping(event: KeyboardEvent): boolean {
  const target = event.target as HTMLElement | null
  if (!target) return false
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName)
}

/**
 * The letter of a keystroke by its place on the keyboard, not by the layout:
 * Ctrl+S on a Russian layout gives `event.key === 'ы'` but `event.code === 'KeyS'`.
 */
export function keyLetter(event: KeyboardEvent): string {
  if (event.code.startsWith('Key')) return event.code.slice(3).toLowerCase()
  return event.key.toLowerCase()
}

/** A dialog, a sheet or a menu is open over the editor: its keys are its own. */
export function overlayOpen(root: HTMLElement | null): boolean {
  const open = document.querySelectorAll<HTMLElement>(
    '[role="dialog"][data-state="open"], [role="alertdialog"][data-state="open"], [role="menu"][data-state="open"]',
  )
  return [...open].some((element) => !root?.contains(element))
}
