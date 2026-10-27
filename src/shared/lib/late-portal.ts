import { type Ref, ref, watch } from 'vue'

/** Longer than the exit animation of a popover or a drawer. */
const EXIT_MS = 400

/**
 * Keeps a layer's portal in the page only while the layer is open (and while it slides out).
 *
 * A Teleport keeps its place in `<body>` from the day it was mounted. A layer whose component lives
 * for the whole session (the sum card in the shell) would open at that early place: under a glance or
 * a sheet opened later, and Reka — which tells «outside» from «inside» by the order of layers in
 * `<body>` — would take a press in that glance for a press inside the card, and the other way round.
 * A portal made when the layer opens lands at the end of `<body>`: the order of opening is the order
 * of the stack.
 */
export function useLatePortal(open: Ref<boolean>): Ref<boolean> {
  const mounted = ref(open.value)
  let timer: ReturnType<typeof setTimeout> | undefined
  watch(open, (value) => {
    clearTimeout(timer)
    if (value) mounted.value = true
    else timer = setTimeout(() => (mounted.value = false), EXIT_MS)
  })
  return mounted
}
