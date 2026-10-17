import type { Component } from 'vue'
import { Bell, Bug, Coins, FolderTree, HardDrive, RefreshCw, Settings2 } from 'lucide-vue-next'

export interface SettingsSection {
  id: string
  /** i18n key of the section's name in the window's sidebar. */
  titleKey: string
}

export interface SettingsTab {
  id: string
  titleKey: string
  icon: Component
  sections: SettingsSection[]
  devOnly?: boolean
}

/**
 * Tabs and their sections in sidebar order; the shell renders a widget per
 * section id. Everything about money (currencies and rates, budget defaults,
 * inflation) sits in one tab, the vault file and what leaves it in another.
 */
export const SETTINGS_TABS: SettingsTab[] = [
  {
    id: 'general',
    titleKey: 'settingsShell.tab.general',
    icon: Settings2,
    sections: [
      { id: 'app', titleKey: 'settingsShell.section.app' },
      { id: 'quick-add', titleKey: 'settingsShell.section.quickAdd' },
      { id: 'tours', titleKey: 'settingsShell.section.tours' },
    ],
  },
  {
    id: 'notifications',
    titleKey: 'settingsShell.tab.notifications',
    icon: Bell,
    sections: [{ id: 'notifications', titleKey: 'settingsShell.section.notifications' }],
  },
  {
    id: 'money',
    titleKey: 'settingsShell.tab.money',
    icon: Coins,
    sections: [
      { id: 'rates', titleKey: 'settingsShell.section.rates' },
      { id: 'budget', titleKey: 'settingsShell.section.budget' },
      { id: 'inflation', titleKey: 'settingsShell.section.inflation' },
    ],
  },
  {
    id: 'categories',
    titleKey: 'settingsShell.tab.categories',
    icon: FolderTree,
    sections: [
      { id: 'categories', titleKey: 'settingsShell.section.categories' },
      { id: 'rules', titleKey: 'settingsShell.section.rules' },
    ],
  },
  {
    id: 'vault',
    titleKey: 'settingsShell.tab.vault',
    icon: HardDrive,
    sections: [
      { id: 'vault', titleKey: 'settingsShell.section.vault' },
      { id: 'enrichment', titleKey: 'settingsShell.section.enrichment' },
    ],
  },
  {
    id: 'sync',
    titleKey: 'settingsShell.tab.sync',
    icon: RefreshCw,
    sections: [
      { id: 'sync', titleKey: 'settingsShell.section.sync' },
      { id: 'sync-devices', titleKey: 'settingsShell.section.syncDevices' },
      { id: 'sync-history', titleKey: 'settingsShell.section.syncHistory' },
    ],
  },
  {
    id: 'dev',
    titleKey: 'settingsShell.tab.dev',
    icon: Bug,
    devOnly: true,
    sections: [{ id: 'devtools', titleKey: 'settingsShell.section.devtools' }],
  },
]

/** Tabs that became sections: old `?settings=budget` links open the tab and scroll there. */
export const SETTINGS_ALIASES: Record<string, { tab: string; section: string }> = {
  budget: { tab: 'money', section: 'budget' },
  inflation: { tab: 'money', section: 'inflation' },
  rates: { tab: 'money', section: 'rates' },
  enrichment: { tab: 'vault', section: 'enrichment' },
}
