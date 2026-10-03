import { onMounted, onUnmounted, ref } from 'vue'

export type SidebarState = 'full' | 'rail' | 'compact'

export const BREAKPOINT_FULL = 1280
export const BREAKPOINT_RAIL = 768

const width = ref(typeof window !== 'undefined' ? window.innerWidth : 1280)

function update() {
  width.value = window.innerWidth
}

/** Reactive viewport width (CSS px). Shared across the app. */
export function useViewport() {
  onMounted(() => {
    update()
    window.addEventListener('resize', update)
  })
  onUnmounted(() => window.removeEventListener('resize', update))
  return width
}

export function sidebarState(w: number): SidebarState {
  if (w >= BREAKPOINT_FULL) return 'full'
  if (w >= BREAKPOINT_RAIL) return 'rail'
  return 'compact'
}