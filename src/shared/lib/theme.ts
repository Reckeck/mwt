import { computed, onMounted, onUnmounted, ref, watch } from 'vue'

import { call } from '@/shared/api/ipc'
import { usePlatform } from '@/shared/lib/platform'
import { applyTheme } from '@/shared/lib/theme/apply'
import { toHex } from '@/shared/lib/theme/contrast'
import { builtinThemes, getTheme } from '@/shared/lib/theme/registry'
import type { Theme, ThemeMode } from '@/shared/lib/theme/types'
import { useSettingsStore } from '@/shared/store/settings'

export type ResolvedTheme = ThemeMode

export type ThemePreference = 'auto' | ThemeMode

export interface ThemeChoice {
  preference: ThemePreference
  themeId: string | null
  themeLightId: string | null
  themeDarkId: string | null
}

/** Read by the inline script in index.html: the theme is right from the first paint. */
export const THEME_STORAGE_KEY = 'trade-app:theme'
export const THEME_ID_STORAGE_KEY = 'trade-app:theme-id'
export const THEME_LIGHT_ID_STORAGE_KEY = 'trade-app:theme-light-id'
export const THEME_DARK_ID_STORAGE_KEY = 'trade-app:theme-dark-id'

function defaultTheme(mode: ThemeMode): Theme {
  const found = builtinThemes.find((theme) => theme.id === `default-${mode}`)
  if (!found) throw new Error(`Default ${mode} theme is not registered`)
  return found
}

/** A stored id counts only when it exists and has the wanted mode; otherwise Default of the mode (TP7). */
export function themeOfMode(id: string | null, mode: ThemeMode): Theme {
  const found = id ? getTheme(id) : undefined
  return found && found.mode === mode ? found : defaultTheme(mode)
}

/** The theme in force (TP9): Light/Dark use `themeId`, Auto picks the pair slot by the system mode. */
export function resolveTheme(choice: ThemeChoice, systemDark: boolean): Theme {
  if (choice.preference === 'auto') {
    return systemDark ? themeOfMode(choice.themeDarkId, 'dark') : themeOfMode(choice.themeLightId, 'light')
  }
  return themeOfMode(choice.themeId, choice.preference)
}

function mirror(key: string, value: string | null) {
  try {
    if (value) localStorage.setItem(key, value)
    else localStorage.removeItem(key)
  } catch {
    // Private storage off: the next start may flash the default theme once.
  }
}

/** The desktop window background follows the theme's `--background`; Rust keeps no colour table (TK7). */
function paintWindow(theme: Theme) {
  try {
    call('set_window_background', { rgb: toHex(theme.tokens['--background']) }).catch(() => undefined)
  } catch {
    // A colour the parser does not know keeps the previous window background.
  }
}

export function useTheme() {
  const settingsStore = useSettingsStore()
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  const systemDark = ref(media.matches)

  const choice = computed<ThemeChoice>(() => ({
    preference: settingsStore.app?.theme ?? 'auto',
    themeId: settingsStore.app?.theme_id ?? null,
    themeLightId: settingsStore.app?.theme_light_id ?? null,
    themeDarkId: settingsStore.app?.theme_dark_id ?? null,
  }))
  const theme = computed(() => resolveTheme(choice.value, systemDark.value))
  const resolved = computed<ResolvedTheme>(() => theme.value.mode)

  function apply() {
    applyTheme(theme.value)
    paintWindow(theme.value)
    if (settingsStore.app) {
      mirror(THEME_STORAGE_KEY, choice.value.preference)
      mirror(THEME_ID_STORAGE_KEY, choice.value.themeId)
      mirror(THEME_LIGHT_ID_STORAGE_KEY, choice.value.themeLightId)
      mirror(THEME_DARK_ID_STORAGE_KEY, choice.value.themeDarkId)
    }
  }

  function onMediaChange(event: MediaQueryListEvent) {
    systemDark.value = event.matches
  }

  watch([theme, choice], apply, { immediate: true })
  // Android draws the status-bar icons itself: dark icons on a light page and the other way round.
  const { isAndroid } = usePlatform()
  watch(
    [resolved, isAndroid],
    ([mode, android]) => {
      if (android) call('set_system_bars', { dark: mode === 'dark' }).catch(() => undefined)
    },
    { immediate: true },
  )
  onMounted(() => {
    systemDark.value = media.matches
    media.addEventListener('change', onMediaChange)
  })
  onUnmounted(() => media.removeEventListener('change', onMediaChange))

  return { resolved, theme }
}
