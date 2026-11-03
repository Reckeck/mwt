/** What the update status means for a person: one view model for the settings section and the notice. */

export type UpdateViewKind = 'idle' | 'checking' | 'upToDate' | 'downloading' | 'ready' | 'available' | 'error'

/** The one thing the person can do about the status. */
export type UpdateAction = 'none' | 'restart' | 'openRelease' | 'openApk' | 'packageManager'

export interface UpdateView {
  kind: UpdateViewKind
  /** The new version, when there is one. */
  version: string | null
  percent: number
  notes: string | null
  action: UpdateAction
  /** The page or file the `open*` actions lead to. */
  url: string | null
  /** The error text of the backend. */
  message: string | null
}

const BASE: UpdateView = {
  kind: 'idle',
  version: null,
  percent: 0,
  notes: null,
  action: 'none',
  url: null,
  message: null,
}

function offerAction(offer: UpdateOffer): Pick<UpdateView, 'action' | 'url'> {
  switch (offer.hint) {
    case 'apk':
      return { action: 'openApk', url: offer.download_url ?? offer.release_url }
    case 'release_page':
      return { action: 'openRelease', url: offer.release_url ?? offer.download_url }
    case 'package_manager':
      return { action: 'packageManager', url: null }
    default:
      return { action: 'none', url: null }
  }
}

export function describeUpdate(status: UpdateStatusDto | undefined): UpdateView {
  const phase = status?.phase
  if (!phase) return BASE
  switch (phase.type) {
    case 'checking':
      return { ...BASE, kind: 'checking' }
    case 'up_to_date':
      return { ...BASE, kind: 'upToDate' }
    case 'downloading':
      return { ...BASE, kind: 'downloading', percent: Math.min(100, Math.max(0, Math.round(phase.percent))) }
    case 'ready':
      return { ...BASE, kind: 'ready', version: phase.version, action: 'restart' }
    case 'available':
      return {
        ...BASE,
        kind: 'available',
        version: phase.offer.version,
        notes: phase.offer.notes,
        ...offerAction(phase.offer),
      }
    case 'error':
      return { ...BASE, kind: 'error', message: phase.message }
    default:
      return BASE
  }
}

/** The notice appears only for a version the person can act on. */
export function noticeVersion(view: UpdateView): string | null {
  return view.kind === 'ready' || view.kind === 'available' ? view.version : null
}
