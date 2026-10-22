import type { ComputedRef, Ref } from 'vue'
import { createContext } from 'reka-ui'

/** localStorage key for the expanded/collapsed state (upstream keeps it in a cookie). */
export const SIDEBAR_STORAGE_KEY = 'trade-app:sidebar'

export function readSidebarOpen(): boolean {
  try {
    return localStorage.getItem(SIDEBAR_STORAGE_KEY) !== 'false'
  } catch {
    return true
  }
}

export function writeSidebarOpen(open: boolean): void {
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, String(open))
  } catch {
    // Storage blocked: the state just won't survive a restart.
  }
}
export const SIDEBAR_WIDTH = '16rem'
export const SIDEBAR_WIDTH_MOBILE = '18rem'
export const SIDEBAR_WIDTH_ICON = '3rem'
/** Below this width the sidebar folds to icons by itself (Shell · 7). */
export const NARROW_QUERY = '(max-width: 1099px)'

export const [useSidebar, provideSidebarContext] = createContext<{
  state: ComputedRef<'expanded' | 'collapsed'>
  open: Ref<boolean>
  setOpen: (value: boolean) => void
  isMobile: Ref<boolean>
  /** The window is below NARROW_QUERY: the open sidebar lies over the page. */
  narrow: Ref<boolean>
  openMobile: Ref<boolean>
  setOpenMobile: (value: boolean) => void
  toggleSidebar: () => void
}>('Sidebar')
