import { computed, onUnmounted, watch } from 'vue'
import { useEventListener, useOnline } from '@vueuse/core'
import { useRoute, useRouter } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'

import { usePriceRefresh } from '@/features/price-refresh'
import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { quickAddOpen } from '@/shared/lib/quick-add'
import { settingsQuery } from '@/shared/lib/settings-link'
import { tourIdOf, useTour } from '@/shared/lib/tour'
import { useVaultStore } from '@/shared/store/vault'

/** The net worth in the header, and whether the app is online. */
export function useConnectivity() {
  // Deduped with the identical query inside the sidebar widgets.
  const summary = useQuery({
    queryKey: ['summary'],
    queryFn: () => call<PortfolioSummary>('get_summary'),
    staleTime: 60_000,
  })
  const isOnline = useOnline()
  return { summary, isOnline }
}

/** Guided tours: «?» starts the page's tour; a first visit starts it once. */
export function useTourAutostart() {
  const route = useRoute()
  const tour = useTour()
  const routeTour = computed(() => tourIdOf(String(route.name ?? '')))
  let tourTimer: ReturnType<typeof setTimeout> | undefined
  watch(
    routeTour,
    (id) => {
      // A pending tour belongs to the screen that scheduled it, never to the next one;
      // one on screen closes with its page.
      clearTimeout(tourTimer)
      tour.stop()
      const wanted = route.query.tour === id
      if (!tour.hasTour(id)) return
      // Let the page render its widgets before pointing at them.
      tourTimer = setTimeout(() => {
        if (routeTour.value !== id || document.querySelector('[role="dialog"]')) return
        // A lesson guides on its own: the general tour of the screen would talk over it.
        if (route.query.lesson && !wanted) return
        // `?tour=<route>` asks for it explicitly (the learning centre, «Что нового»).
        if (wanted) tour.start(id)
        else tour.autoStart(id)
      }, 1200)
    },
    { immediate: true },
  )
  onUnmounted(() => clearTimeout(tourTimer))
  return { tour, routeTour }
}

/**
 * The WebView's own menu (Reload, Inspect…) is not the app's: every block has
 * its own right-click menu. Text fields keep the native one for copy and paste.
 */
export function useNativeMenuBlock() {
  useEventListener(window, 'contextmenu', (event: MouseEvent) => {
    const target = event.target as HTMLElement | null
    if (target?.closest('input, textarea, [contenteditable="true"], .cm-editor')) return
    event.preventDefault()
  })
}

/**
 * The shell's own actions — settings, lock, quick add — and the tray menu and
 * global shortcut that trigger them from outside the window.
 */
export function useShellActions() {
  const route = useRoute()
  const router = useRouter()
  const vault = useVaultStore()

  // 'list': the phone opens on the list of groups, the desktop on the first group.
  function openSettings(tab = 'list') {
    void router.push({ query: settingsQuery(route.query, tab) })
  }
  async function lock() {
    // The tray and the shortcut may ask for it: a vault without a password cannot be locked.
    if (!vault.encrypted) return
    await vault.lock()
  }

  // The system-wide shortcut (Settings) brings the window up and lands here.
  useBackendEvent('quick-add', () => {
    quickAddOpen.value = true
  })
  useBackendEvent<string>('tray-navigate', (target) => {
    if (target === 'settings') openSettings()
    else void router.push({ name: target })
  })
  const priceRefresh = usePriceRefresh()
  useBackendEvent('refresh-prices', () => priceRefresh.mutate())
  useBackendEvent('toggle-lock', () => void lock())

  return { quickAddOpen, openSettings, lock }
}
