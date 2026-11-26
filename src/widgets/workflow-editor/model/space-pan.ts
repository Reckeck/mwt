import { onBeforeUnmount, onMounted, ref } from 'vue'

import { isTyping } from './hotkeys'

interface Viewport {
  x: number
  y: number
  zoom: number
}

/** A control that Space presses: Space must keep doing that there. */
function isControl(event: KeyboardEvent): boolean {
  const target = event.target as HTMLElement | null
  return Boolean(target?.closest('button, a, [role="button"], [role="switch"], [role="checkbox"]'))
}

/**
 * Hold Space and drag: the canvas moves along x and y from any point, over
 * nodes too (they do not move). Ctrl+Space stays the quick add, and Space in
 * a field or on a button does what it always does.
 */
export function useSpacePan(viewport: { get: () => Viewport; set: (next: Viewport) => void }) {
  const held = ref(false)
  const panning = ref(false)
  let start: { pointerX: number; pointerY: number; from: Viewport } | null = null

  function move(event: PointerEvent) {
    if (!start) return
    viewport.set({
      x: start.from.x + event.clientX - start.pointerX,
      y: start.from.y + event.clientY - start.pointerY,
      zoom: start.from.zoom,
    })
  }

  function stop() {
    start = null
    panning.value = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', stop)
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.code !== 'Space' || event.ctrlKey || event.metaKey || event.altKey) return
    if (isTyping(event) || isControl(event)) return
    // No page scroll while the canvas is being held.
    event.preventDefault()
    held.value = true
  }

  function onKeyup(event: KeyboardEvent) {
    if (event.code === 'Space') held.value = false
  }

  function onBlur() {
    held.value = false
    stop()
  }

  /** On the canvas, in the capture phase: a press with Space held pans instead of dragging a node. */
  function onPress(event: MouseEvent | PointerEvent) {
    if (!held.value || event.button !== 0) return
    event.preventDefault()
    event.stopPropagation()
    if (event.type !== 'pointerdown' || start) return
    start = { pointerX: event.clientX, pointerY: event.clientY, from: viewport.get() }
    panning.value = true
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', stop)
  }

  onMounted(() => {
    window.addEventListener('keydown', onKeydown)
    window.addEventListener('keyup', onKeyup)
    window.addEventListener('blur', onBlur)
  })
  onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeydown)
    window.removeEventListener('keyup', onKeyup)
    window.removeEventListener('blur', onBlur)
    stop()
  })

  return { held, panning, onPress }
}
