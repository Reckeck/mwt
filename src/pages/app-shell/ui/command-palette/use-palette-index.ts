import { type Ref, computed } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { SETTINGS_GROUPS } from '@/widgets/settings-shell'

export interface PaletteEntry {
  id: string
  label: string
  /** Hidden words the search also matches (`Password` finds the vault). */
  keywords: string
  to: RouteLocationRaw
}

/**
 * What the palette finds beyond the screens and wallets: every settings
 * section and every sub-tab.
 */
export function usePaletteIndex(screenNames: Ref<string[]>) {
  const { t, te } = useI18n()
  const route = useRoute()
  const router = useRouter()
  const words = (key: string) => (te(`palette.keywords.${key}`) ? t(`palette.keywords.${key}`) : '')

  const settings = computed<PaletteEntry[]>(() =>
    SETTINGS_GROUPS.filter((group) => !group.devOnly).flatMap((group) =>
      group.tabs.flatMap((tab) =>
        tab.sections.map((section) => ({
          id: `settings-${group.id}-${section.id}`,
          label: [
            group.titleKey,
            group.tabs.length > 1 ? tab.titleKey : '',
            group.tabs.length === 1 || tab.sections.length > 1 ? section.titleKey : '',
          ]
            .filter(Boolean)
            .map((key) => t(key))
            .join(' › '),
          keywords: `${t('settingsShell.title')} ${words(section.id)}`,
          to: { query: { ...route.query, settings: group.id, section: section.id } },
        })),
      ),
    ),
  )

  /** «Planning › Goals»: a tab's own name after its section's, from the key the page itself uses. */
  const TAB_PREFIXES = ['planning.tab', 'transactions.tab', 'workflow.tab', 'analytics.group']
  function tabTitle(name: string, section: string): string {
    const tab = name.split('-').slice(1).join('-') || 'list'
    const key = TAB_PREFIXES.map((prefix) => `${prefix}.${tab}`).find((candidate) => te(candidate))
    return key ? `${t(section)} › ${t(key)}` : t(section)
  }

  /** Named routes with a title that are not in the sidebar: tabs inside sections. */
  const tabs = computed<PaletteEntry[]>(() =>
    router
      .getRoutes()
      .filter((entry) => typeof entry.name === 'string' && entry.meta?.i18n && !entry.path.includes(':'))
      .filter((entry) => !screenNames.value.includes(String(entry.name)))
      .map((entry) => ({
        id: `tab-${String(entry.name)}`,
        label: tabTitle(String(entry.name), String(entry.meta.i18n)),
        keywords: words(String(entry.name)),
        to: { name: String(entry.name) },
      })),
  )

  return { settings, tabs }
}
