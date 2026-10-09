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
export const SIDEBAR_KEYBOARD_SHORTCUT = 'b'

export const [useSidebar, provideSidebarContext] = createContext<{
  state: ComputedRef<'expanded' | 'collapsed'>
  open: Ref<boolean>
  setOpen: (value: boolean) => void
  isMobile: Ref<boolean>
  openMobile: Ref<boolean>
  setOpenMobile: (value: boolean) => void
  toggleSidebar: () => void
}>('Sidebar')
