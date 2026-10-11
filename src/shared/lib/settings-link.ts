import type { LocationQueryRaw } from 'vue-router'

/** Query that opens the settings window on a tab (and section): `?settings=categories&section=rules`. */
export function settingsQuery(current: LocationQueryRaw, tab = 'general', section?: string): LocationQueryRaw {
  const next: LocationQueryRaw = { ...current, settings: tab }
  if (section) next.section = section
  else delete next.section
  return next
}

/** The same query without the settings window. */
export function withoutSettings(current: LocationQueryRaw): LocationQueryRaw {
  const { settings: _settings, section: _section, ...rest } = current
  return rest
}
