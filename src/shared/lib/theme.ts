import { computed, onMounted, onUnmounted, watch } from 'vue'

import { call } from '@/shared/api/ipc'
import { usePlatform } from '@/shared/lib/platform'
import { useSettingsStore } from '@/shared/store/settings'

export type ResolvedTheme = 'light' | 'dark'

const media = window.matchMedia('(prefers-color-scheme: dark)')

export function useTheme() {
  const settingsStore = useSettingsStore()

  const resolved = computed<ResolvedTheme>(() => {
    const preference = settingsStore.app?.theme ?? 'auto'
    if (preference === 'dark') return 'dark'
    if (preference === 'light') return 'light'
    return media.matches ? 'dark' : 'light'
  })

  function apply() {
    const root = document.documentElement
    root.classList.toggle('dark', resolved.value === 'dark')
    root.setAttribute('data-theme', resolved.value)
    root.style.colorScheme = resolved.value
  }

  function onMediaChange() {
    apply()
  }

  watch(resolved, apply, { immediate: true })
  // Android draws the status-bar icons itself: dark icons on a light page and the other way round.
  const { isAndroid } = usePlatform()
  watch(
    [resolved, isAndroid],
    ([theme, android]) => {
      if (android) call('set_system_bars', { dark: theme === 'dark' }).catch(() => undefined)
    },
    { immediate: true },
  )
  onMounted(() => media.addEventListener('change', onMediaChange))
  onUnmounted(() => media.removeEventListener('change', onMediaChange))

  return { resolved }
}
