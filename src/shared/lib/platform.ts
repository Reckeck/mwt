import { computed, ref } from 'vue'

import { call } from '@/shared/api/ipc'

const MOBILE = ['android', 'ios']

/** The OS once asked; `null` until the answer is in. One request for the whole app. */
const os = ref<string | null>(null)
let asked = false

/**
 * Where the app runs. Tray, autostart, the global shortcut and the window
 * exist only on a desktop; until the answer arrives nothing desktop-only shows.
 */
export function usePlatform() {
  if (!asked) {
    asked = true
    call<string>('app_platform')
      .then((value) => {
        os.value = value
      })
      .catch(() => {
        os.value = 'unknown'
      })
  }
  return {
    os,
    isMobile: computed(() => os.value !== null && MOBILE.includes(os.value)),
    isAndroid: computed(() => os.value === 'android'),
    isDesktop: computed(() => os.value !== null && !MOBILE.includes(os.value) && os.value !== 'unknown'),
  }
}
