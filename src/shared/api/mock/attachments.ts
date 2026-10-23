/** Files and links of operations and goals (`service/attachments.rs`), in memory. */

import { txId } from '@/shared/lib/transactions'
import { mockTransactions } from './demo'
import { goals } from './planning/state'

const MAX_BYTES = 25 * 1024 * 1024

export const mockAttachments: Attachment[] = []
/** The bytes of each file, base64, by attachment id. */
export const mockAttachmentData = new Map<string, string>()

let seeded = false

/** A receipt and a link on the two latest payments, so the paperclip shows in `pnpm dev`. */
function seed() {
  if (seeded) return
  seeded = true
  const payments = mockTransactions.filter((tx) => 'payment' in tx).slice(-2)
  const [first, second] = payments
  const at = new Date().toISOString()
  if (first) {
    const id = crypto.randomUUID()
    mockAttachmentData.set(id, btoa('%PDF-1.7 mock receipt'))
    mockAttachments.push({
      id,
      owner: { kind: 'tx', id: txId(first) },
      kind: 'file',
      title: 'чек-перекрёсток.pdf',
      url: null,
      mime: 'application/pdf',
      size: 248_000,
      sha256: null,
      created_at: at,
      removed_at: null,
    })
  }
  if (second) {
    mockAttachments.push({
      id: crypto.randomUUID(),
      owner: { kind: 'tx', id: txId(second) },
      kind: 'link',
      title: 'Электронный чек',
      url: 'https://check.ofd.ru/rec/mock',
      mime: null,
      size: 0,
      sha256: null,
      created_at: at,
      removed_at: null,
    })
  }
}

export function liveAttachments(): Attachment[] {
  seed()
  return mockAttachments.filter((item) => !item.removed_at)
}

/** Operations with at least one file or link (the «С вложениями» filter). */
export function mockAttachedTxIds(): Set<string> {
  return new Set(
    liveAttachments()
      .filter((item) => item.owner.kind === 'tx')
      .map((item) => item.owner.id),
  )
}

export function checkOwner(owner: EntityRef) {
  const exists =
    owner.kind === 'tx'
      ? mockTransactions.some((tx) => txId(tx) === owner.id)
      : owner.kind === 'goal' && goals.some((goal) => goal.id === owner.id)
  if (!exists) throw { code: 'MISSING', message: `${owner.kind} ${owner.id}` }
}

export function normalizeUrl(raw: string): string {
  const url = raw.trim()
  if (!url || /\s/.test(url)) throw { code: 'INVALID_INPUT', message: 'not an address' }
  if (/^(https?:\/\/|mailto:)/i.test(url)) return url
  if (url.includes('://') || !url.includes('.')) throw { code: 'INVALID_INPUT', message: 'not a web address' }
  return `https://${url}`
}

export function checkSize(name: string, base64: string) {
  if (!name.trim()) throw { code: 'INVALID_INPUT', message: 'a file needs a name' }
  const size = Math.floor((base64.length * 3) / 4)
  if (!size) throw { code: 'INVALID_INPUT', message: 'the file is empty' }
  if (size > MAX_BYTES) throw { code: 'TOO_LARGE', message: `${name}: more than 25 MB` }
  return size
}
