type Translate = (key: string, params?: Record<string, unknown>) => string

/** The other device cut this one off on purpose (paused it, or forgot it). */
const CUT_CODES = new Set(['SYNC_PAUSED_BY_PEER', 'SYNC_FORGOTTEN_BY_PEER'])

/**
 * The sentence under a sync error that names a device: who cut this one off,
 * or whose app version is incompatible. `undefined`: the plain text of the code.
 */
export function syncErrorDescription(event: SyncErrorEvent, t: Translate): string | undefined {
  if (CUT_CODES.has(event.code)) return t(`errors.${event.code}`, { name: event.peer_name ?? '' })
  if (event.code === 'SECURITY_LEVEL_REQUIRED')
    return t('errors.SYNC_SECURITY_LEVEL_REQUIRED', { name: event.peer_name ?? '' })
  if (event.code === 'SYNC_VERSION_MISMATCH' && event.peer_name) {
    const own = event.own_version ?? ''
    return event.peer_version
      ? t('errors.SYNC_VERSION_MISMATCH_NAMED', { name: event.peer_name, version: event.peer_version, own })
      : t('errors.SYNC_VERSION_MISMATCH_OLD', { name: event.peer_name, own })
  }
  return undefined
}
