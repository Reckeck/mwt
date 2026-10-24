import type { SettingsGroup, SettingsTab } from './settings-shell.constants'

export interface SettingsTarget {
  group: SettingsGroup
  tab: SettingsTab
  /** The section to scroll to, when the link named one that exists. */
  section?: string
}

/**
 * `?settings=<group>&section=<id>` to a group, tab and section. An unknown
 * group opens the first one; an unknown section, the group's first tab.
 */
export function resolveSettingsTarget(groups: SettingsGroup[], settings: unknown, section: unknown): SettingsTarget {
  const group = groups.find((item) => item.id === settings) ?? groups[0]
  const tab = group.tabs.find((item) => item.sections.some((entry) => entry.id === section)) ?? group.tabs[0]
  return { group, tab, section: tab.sections.some((item) => item.id === section) ? String(section) : undefined }
}
