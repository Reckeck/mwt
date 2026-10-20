import { type Ref, computed } from 'vue'
import { type RouteLocationRaw, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { SETTINGS_TABS } from '@/widgets/settings-shell'
import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'
import { useContacts } from '@/shared/api/queries'

export interface PaletteEntry {
  id: string
  label: string
  /** Hidden words the search also matches («пароль» finds the vault). */
  keywords: string
  to: RouteLocationRaw
}

/**
 * What the palette finds beyond the screens and wallets: every settings
 * section, every sub-tab, the workflows and the contacts by name. The lists
 * load only once the palette is open.
 */
export function usePaletteIndex(open: Ref<boolean>, screenNames: Ref<string[]>) {
  const { t, te } = useI18n()
  const route = useRoute()
  const router = useRouter()
  const words = (key: string) => (te(`palette.keywords.${key}`) ? t(`palette.keywords.${key}`) : '')

  const settings = computed<PaletteEntry[]>(() =>
    SETTINGS_TABS.filter((tab) => !tab.devOnly).flatMap((tab) =>
      tab.sections.map((section) => ({
        id: `settings-${tab.id}-${section.id}`,
        label: tab.sections.length > 1 ? `${t(tab.titleKey)} › ${t(section.titleKey)}` : t(tab.titleKey),
        keywords: `${t('settingsShell.title')} ${words(section.id)}`,
        to: { query: { ...route.query, settings: tab.id, section: section.id } },
      })),
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

  const workflowList = useQuery({
    queryKey: WORKFLOW_KEYS.list,
    queryFn: () => call<Workflow[]>('list_workflows'),
    enabled: open,
  })
  const workflows = computed<PaletteEntry[]>(() =>
    (workflowList.data.value ?? []).map((workflow) => ({
      id: `workflow-${workflow.id}`,
      label: workflow.name,
      keywords: 'workflow',
      to: { name: 'workflow-edit', params: { id: workflow.id } },
    })),
  )

  const contactList = useContacts(open)
  const contacts = computed<PaletteEntry[]>(() =>
    (contactList.data.value ?? []).map((contact) => ({
      id: `contact-${contact.id}`,
      label: contact.name,
      keywords: t('palette.contacts'),
      to: { name: 'transactions', query: { contact: contact.id } },
    })),
  )

  return { settings, tabs, workflows, contacts }
}
