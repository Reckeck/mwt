import { numberFormat } from '@/shared/lib/format'

/** What the row's icon and its first word say: «PDF», «Картинка», «Ссылка». */
export type AttachmentLook = 'pdf' | 'image' | 'document' | 'link'

export function attachmentLook(item: Pick<Attachment, 'kind' | 'mime' | 'title'>): AttachmentLook {
  if (item.kind === 'link') return 'link'
  const mime = item.mime ?? ''
  if (mime === 'application/pdf' || /\.pdf$/i.test(item.title)) return 'pdf'
  if (mime.startsWith('image/') || /\.(png|jpe?g|gif|webp|heic|bmp)$/i.test(item.title)) return 'image'
  return 'document'
}

/** «84 КБ», «1,2 МБ»: one decimal only under ten. */
export function formatBytes(bytes: number, units: { b: string; kb: string; mb: string }): string {
  if (bytes < 1024) return `${bytes} ${units.b}`
  const kb = bytes / 1024
  if (kb < 1024) return `${numberFormat({ maximumFractionDigits: 0 }).format(kb)} ${units.kb}`
  const mb = kb / 1024
  return `${numberFormat({ maximumFractionDigits: mb < 10 ? 1 : 0 }).format(mb)} ${units.mb}`
}

/** «t.me/skycapital/1842»: a link without its scheme and trailing slash. */
export function linkShown(url: string): string {
  return url.replace(/^(https?:\/\/|mailto:)/i, '').replace(/\/$/, '')
}

/** How many attachments each owner has, from one list of them all. */
export function attachmentCounts(items: readonly Attachment[]): Map<string, number> {
  const counts = new Map<string, number>()
  for (const item of items) counts.set(item.owner.id, (counts.get(item.owner.id) ?? 0) + 1)
  return counts
}
