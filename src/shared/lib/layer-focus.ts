/**
 * Layers over layers: where the focus goes when the top one closes (the stacking order is `late-portal.ts`).
 *
 * Reka sends the focus back to the control that opened a layer. A layer opened from a
 * sum (`MoneyCardHost`), by a route, from a hover or a menu that unmounts with its row has no such control:
 * the focus fell to `<body>`, the keyboard (and «Esc» / «Back», which aim at the focused node) lost
 * the layer under it. One observer for every layer of the app: it remembers what had the focus when
 * a layer took it and, once the layer is gone and the focus is nowhere, gives it back — to that
 * element, else to the top layer that is left.
 * Esc / «Back» themselves are in `back-stack.ts`; clicks in a layer over a layer are Reka's own
 * (a higher layer is not «outside» for the ones under it).
 */
const LAYER = '[data-dismissable-layer]'
/** Reka returns the focus itself on the next tick; we only act on what it left behind. */
const SETTLE_MS = 80

const openers = new Map<Element, Element | null>()
let lastFocus: Element | null = null

function lost(): boolean {
  const active = document.activeElement
  return !active || active === document.body || !active.isConnected
}

function remember(target: EventTarget | null) {
  if (!(target instanceof Element) || target === document.body) return
  const layer = target.closest(LAYER)
  if (layer && !openers.has(layer)) {
    // The first focus inside a new layer: the element that had it before is the opener.
    openers.set(layer, lastFocus?.isConnected && !layer.contains(lastFocus) ? lastFocus : null)
  }
  lastFocus = target
}

function topLayer(): HTMLElement | null {
  const layers = [...document.querySelectorAll<HTMLElement>(LAYER)].filter(
    (layer) => layer.getAttribute('data-state') !== 'closed' && layer.getClientRects().length > 0,
  )
  return layers.at(-1) ?? null
}

function restore(opener: Element | null | undefined) {
  if (!lost()) return
  const layer = topLayer()
  // An opener under a layer that is still open would take the focus out of it.
  const target = opener?.isConnected && (!layer || layer.contains(opener) || !opener.closest(LAYER)) ? opener : layer
  if (target instanceof HTMLElement) target.focus({ preventScroll: true })
}

/** Start watching the layers; returns the stop function. Called once, from `App.vue`. */
export function installLayerFocus(): () => void {
  const onFocusIn = (event: FocusEvent) => remember(event.target)
  const onPress = () => {
    // A press or a key is what opens most layers: the focus before it is the opener.
    const active = document.activeElement
    if (active && active !== document.body) lastFocus = active
  }
  const observer = new MutationObserver(() => {
    const gone = [...openers.keys()].filter((layer) => !layer.isConnected)
    if (!gone.length) return
    const opener = openers.get(gone.at(-1) as Element)
    for (const layer of gone) openers.delete(layer)
    setTimeout(() => restore(opener), SETTLE_MS)
  })
  document.addEventListener('focusin', onFocusIn, true)
  document.addEventListener('pointerdown', onPress, true)
  document.addEventListener('keydown', onPress, true)
  observer.observe(document.body, { childList: true })
  return () => {
    document.removeEventListener('focusin', onFocusIn, true)
    document.removeEventListener('pointerdown', onPress, true)
    document.removeEventListener('keydown', onPress, true)
    observer.disconnect()
    openers.clear()
  }
}
