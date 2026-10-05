import { computed, ref, watch } from 'vue'

import { detectLocalePreference, i18n, persistLocale, resolveLocale } from '@/i18n'
import type { LocalePreference, ResolvedLocale } from '@/i18n'
import { useSettingsStore } from '@/stores/settings'

/**
 * The concrete locale the UI is currently rendering in.
 *
 * Lives at module scope so that plain formatter helpers (`formatMoney`,
 * `formatDate`, …) can read it without prop-drilling. Because they read it
 * while a template renders, the dependency is tracked and every number/date
 * in the UI re-formats the moment the language changes.
 */
export const activeLocale = ref<ResolvedLocale>(resolveLocale(detectLocalePreference()))

/** Push a preference into vue-i18n and the shared `activeLocale` ref. */
export function applyLocale(pref: LocalePreference): ResolvedLocale {
  const resolved = resolveLocale(pref)
  activeLocale.value = resolved
  i18n.global.locale.value = resolved
  return resolved
}

/**
 * Change the language immediately and persist it to the app settings.
 *
 * The app config is the source of truth, but localStorage is written too so a
 * cold start shows the right language before the settings file has been read —
 * and before any vault is unlocked.
 */
export async function setLocalePreference(pref: LocalePreference): Promise<void> {
  persistLocale(pref)
  applyLocale(pref)

  const settingsStore = useSettingsStore()
  if (settingsStore.app) {
    await settingsStore.updateApp({ locale: pref })
  }
}

/**
 * Keeps vue-i18n, `activeLocale` and localStorage in sync with the app-level
 * language preference. Call once from `App.vue`, mirroring `useTheme()`.
 */
export function useLocale() {
  const settingsStore = useSettingsStore()

  const preference = computed<LocalePreference>(() => settingsStore.app?.locale ?? detectLocalePreference())

  watch(
    preference,
    (pref) => {
      applyLocale(pref)
      persistLocale(pref)
    },
    { immediate: true },
  )

  return { preference, activeLocale }
}
