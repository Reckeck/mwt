import { type Ref, computed } from 'vue'
import { type RouteLocationRaw, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

export interface PaletteEntry {
  id: string
  label: string
  /** Hidden words the search also matches (`Password` finds the vault). */
  keywords: string
  to: RouteLocationRaw
}

/**
 * What the palette finds beyond the screens and wallets: every sub-tab.
 */
export function usePaletteIndex(screenNames: Ref<string[]>) {
  const { t, te } = useI18n()
  const router = useRouter()
  const words = (key: string) => (te(`palette.keywords.${key}`) ? t(`palette.keywords.${key}`) : '')

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

  return { tabs }
}
