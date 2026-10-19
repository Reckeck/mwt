/**
 * What the system «Back» closes before it leaves the page. Reka layers
 * (dialogs, sheets, menus, lists) close on Escape themselves; anything else
 * that floats over the page (the tour) registers here while it is open.
 */
const layers: Array<() => void> = []

/** Register an open layer; call the returned function once it closes. */
export function pushBackLayer(close: () => void): () => void {
  layers.push(close)
  return () => {
    const index = layers.lastIndexOf(close)
    if (index >= 0) layers.splice(index, 1)
  }
}

/** Close the top layer, if any: `true` when something closed. */
export function closeTopLayer(): boolean {
  const close = layers.pop()
  if (close) {
    close()
    return true
  }
  if (!document.querySelector('[data-dismissable-layer]')) return false
  // The topmost Reka layer is the one that reacts to Escape.
  const target = document.activeElement instanceof HTMLElement ? document.activeElement : document.body
  target.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
  return true
}
