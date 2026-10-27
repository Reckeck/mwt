import type { Component } from 'vue'
import { Book, ChartPie, Code, Database, Sun, Wifi } from 'lucide-vue-next'

/** `?settings=list`: on a phone the list of groups, before any group is opened. */
export const SETTINGS_LIST = 'list'

export interface SettingsSection {
  id: string
  /** i18n key of the section's name. */
  titleKey: string
  /** Exists only on a desktop (the global shortcut, the tray). */
  desktopOnly?: boolean
}

export interface SettingsTab {
  id: string
  titleKey: string
  /** A shorter name for a phone, where the strip of tabs is narrow. */
  shortKey?: string
  sections: SettingsSection[]
}

export interface SettingsGroup {
  id: string
  titleKey: string
  /** i18n key of the small line under the group's name. */
  hintKey: string
  icon: Component
  tabs: SettingsTab[]
  devOnly?: boolean
}

const section = (id: string, titleKey: string, desktopOnly?: boolean): SettingsSection => ({
  id,
  titleKey: `settingsShell.section.${titleKey}`,
  ...(desktopOnly ? { desktopOnly } : {}),
})

/**
 * Groups of the left menu and the real tabs inside each; the shell renders a
 * widget per section id, one tab at a time. A group with one tab shows no strip.
 */
export const SETTINGS_GROUPS: SettingsGroup[] = [
  {
    id: 'app',
    titleKey: 'settingsShell.group.app',
    hintKey: 'settingsShell.group.appHint',
    icon: Sun,
    tabs: [
      {
        id: 'general',
        titleKey: 'settingsShell.tab.general',
        sections: [
          section('app', 'app'),
          section('device-app', 'deviceApp', true),
          section('notifications', 'notifications'),
        ],
      },
      {
        id: 'keys',
        titleKey: 'settingsShell.tab.keys',
        sections: [section('quick-add', 'quickAdd', true), section('hotkeys', 'hotkeys')],
      },
    ],
  },
  {
    id: 'accounting',
    titleKey: 'settingsShell.group.accounting',
    hintKey: 'settingsShell.group.accountingHint',
    icon: ChartPie,
    tabs: [
      {
        id: 'general',
        titleKey: 'settingsShell.tab.general',
        sections: [
          section('currencies', 'currencies'),
          section('direction', 'direction'),
          section('rates', 'ratesTable'),
        ],
      },
      {
        id: 'money',
        titleKey: 'settingsShell.tab.money',
        sections: [section('budget', 'budget'), section('inflation', 'inflation')],
      },
      { id: 'categories', titleKey: 'settingsShell.tab.categories', sections: [section('categories', 'categories')] },
      {
        id: 'rules',
        titleKey: 'settingsShell.tab.rules',
        shortKey: 'settingsShell.tab.rulesShort',
        sections: [section('rules', 'rules')],
      },
    ],
  },
  {
    id: 'vault',
    titleKey: 'settingsShell.group.vault',
    hintKey: 'settingsShell.group.vaultHint',
    icon: Database,
    tabs: [{ id: 'file', titleKey: 'settingsShell.tab.file', sections: [section('vault', 'vault')] }],
  },
  {
    id: 'sync',
    titleKey: 'settingsShell.group.sync',
    hintKey: 'settingsShell.group.syncHint',
    icon: Wifi,
    tabs: [
      { id: 'sync', titleKey: 'settingsShell.tab.general', sections: [section('sync', 'sync')] },
      { id: 'devices', titleKey: 'settingsShell.tab.devices', sections: [section('sync-devices', 'syncDevices')] },
      { id: 'history', titleKey: 'settingsShell.tab.history', sections: [section('sync-history', 'syncHistory')] },
      { id: 'transfer', titleKey: 'settingsShell.tab.transfer', sections: [section('sync-transfer', 'syncTransfer')] },
    ],
  },
  {
    id: 'help',
    titleKey: 'settingsShell.group.help',
    hintKey: 'settingsShell.group.helpHint',
    icon: Book,
    tabs: [
      {
        id: 'tours',
        titleKey: 'settingsShell.tab.tours',
        sections: [section('tours', 'tours'), section('whats-new', 'whatsNew')],
      },
    ],
  },
  {
    id: 'dev',
    titleKey: 'settingsShell.group.dev',
    hintKey: 'settingsShell.group.devHint',
    icon: Code,
    devOnly: true,
    tabs: [
      {
        id: 'dev',
        titleKey: 'settingsShell.tab.dev',
        sections: [section('devtools', 'devtools'), section('enrichment', 'enrichment')],
      },
    ],
  },
]
