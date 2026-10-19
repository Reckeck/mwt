import { onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { call, onBackButton } from '@/shared/api/ipc'
import { closeTopLayer } from '@/shared/lib/back-stack'
import { notify } from '@/shared/lib/notify'

/** A second «Back» on the home screen within this time hides the app. */
const AGAIN_MS = 2000

/**
 * The Android «Back»: first whatever floats over the page (a dialog, a sheet,
 * the mobile menu, the settings window, a menu, the tour), then the previous
 * page; on the home screen a second press within two seconds hides the app.
 * Leaving an unsaved workflow is still asked by its own route guard.
 */
export function useBackButton() {
  const route = useRoute()
  const router = useRouter()
  let armedAt = 0
  let unlisten: (() => void) | null = null
  let gone = false

  function onBack(canGoBack: boolean) {
    if (closeTopLayer()) return
    if (route.name !== 'dashboard' && (canGoBack || window.history.length > 1)) {
      router.back()
      return
    }
    if (route.name !== 'dashboard') {
      void router.push({ name: 'dashboard' })
      return
    }
    const now = Date.now()
    if (now - armedAt < AGAIN_MS) {
      armedAt = 0
      call('move_to_back').catch(() => undefined)
      return
    }
    armedAt = now
    notify.info('app.backAgainToMinimize')
  }

  onMounted(async () => {
    const stop = await onBackButton(onBack)
    if (gone) stop()
    else unlisten = stop
  })
  onUnmounted(() => {
    gone = true
    unlisten?.()
  })
}
