import type { Component } from 'vue'
import { Bug, Cpu, FolderTree, Landmark, Percent, Radar, Settings2 } from 'lucide-vue-next'

export interface SettingsSection {
  id: string
  /** i18n key of the section title (also its sub-tab in the sidebar). */
  titleKey: string
}

export interface SettingsTab {
  id: string
  titleKey: string
  icon: Component
  sections: SettingsSection[]
  devOnly?: boolean
}

/** Tabs and their sections in sidebar order; the shell renders a widget per section id. */
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
    id: 'vault',
    titleKey: 'settingsShell.tab.vault',
    icon: Cpu,
    sections: [{ id: 'vault', titleKey: 'settingsShell.section.vault' }],
  },
  {
    id: 'budget',
    titleKey: 'settingsShell.tab.budget',
    icon: Landmark,
    sections: [{ id: 'budget', titleKey: 'settingsShell.section.budget' }],
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
    id: 'inflation',
    titleKey: 'settingsShell.tab.inflation',
    icon: Percent,
    sections: [{ id: 'inflation', titleKey: 'settingsShell.section.inflation' }],
  },
  {
    id: 'enrichment',
    titleKey: 'settingsShell.tab.enrichment',
    icon: Radar,
    sections: [{ id: 'enrichment', titleKey: 'settingsShell.section.enrichment' }],
  },
  {
    id: 'dev',
    titleKey: 'settingsShell.tab.dev',
    icon: Bug,
    devOnly: true,
    sections: [{ id: 'devtools', titleKey: 'settingsShell.section.devtools' }],
  },
]
