import { onScopeDispose } from 'vue'

const typing = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))

/**
 * Esc or Alt ← run `back`, unless a layer (dialog, menu, sheet) is open or
 * the user is typing: Esc there closes the layer or leaves the field.
 */
export function useBackKeys(back: () => void): void {
  const listener = (event: KeyboardEvent) => {
    if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.shiftKey) return
    const esc = event.key === 'Escape' && !event.altKey
    const altLeft = event.key === 'ArrowLeft' && event.altKey
    if (!esc && !altLeft) return
    if (typing(event.target) || document.querySelector('[data-dismissable-layer]')) return
    event.preventDefault()
    back()
  }
  window.addEventListener('keydown', listener)
  onScopeDispose(() => window.removeEventListener('keydown', listener))
}
