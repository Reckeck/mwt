import { computed, onMounted, onUnmounted, watch } from 'vue'

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
  onMounted(() => media.addEventListener('change', onMediaChange))
  onUnmounted(() => media.removeEventListener('change', onMediaChange))

  return { resolved }
}
