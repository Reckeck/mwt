/** Minimal shape of vue-i18n's `t`. */
type Translate = (key: string) => string

/**
 * The name of a record an id points at. Once the list is loaded, an id that no longer resolves
 * (deleted on another device and merged) shows a placeholder, not the raw id; while the list
 * is still loading the id stays, so nothing flashes.
 */
export function nameOrDeleted(name: string | undefined, loaded: boolean, id: string, t: Translate): string {
  if (name !== undefined || !id) return name ?? id
  return loaded ? t('record.deleted') : id
}
