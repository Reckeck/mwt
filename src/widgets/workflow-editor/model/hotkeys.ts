/** Keyboard shortcuts of the editor, for the help card and the handler. */
export const EDITOR_HOTKEYS = [
  { keys: 'Ctrl+Space', action: 'quickAdd' },
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
