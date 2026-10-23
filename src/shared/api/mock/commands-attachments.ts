/** Commands: files and links of operations and goals (`commands/attachments.rs`). */

import { resolve } from './core'
import type { MockCommands } from './core'
import {
  checkOwner,
  checkSize,
  liveAttachments,
  mockAttachmentData,
  mockAttachments,
  normalizeUrl,
} from './attachments'

function find(id: string, removed = false): Attachment {
  const item = mockAttachments.find((entry) => entry.id === id && (removed || !entry.removed_at))
  if (!item) throw { code: 'MISSING', message: `attachment ${id}` }
  return item
}

const sameOwner = (left: EntityRef, right: EntityRef) => left.kind === right.kind && left.id === right.id

export const attachmentCommands: MockCommands = {
  list_attachments: ({ body }) => {
    const { owner } = (body ?? {}) as ListAttachmentsInput
    return resolve(liveAttachments().filter((item) => !owner || sameOwner(item.owner, owner)))
  },
  add_attachment_link: ({ body }) => {
    const input = body as AddAttachmentLinkInput
    checkOwner(input.owner)
    const url = normalizeUrl(input.url)
    const item: Attachment = {
      id: crypto.randomUUID(),
      owner: input.owner,
      kind: 'link',
      title: input.title.trim() || url,
      url,
      mime: null,
      size: 0,
      sha256: null,
      created_at: new Date().toISOString(),
      removed_at: null,
    }
    mockAttachments.push(item)
    return resolve(item)
  },
  add_attachment_file: ({ body }) => {
    const input = body as AddAttachmentFileInput
    checkOwner(input.owner)
    const size = checkSize(input.name, input.data_base64)
    const same = liveAttachments().find(
      (item) => sameOwner(item.owner, input.owner) && mockAttachmentData.get(item.id) === input.data_base64,
    )
    if (same) return resolve(same)
    const item: Attachment = {
      id: crypto.randomUUID(),
      owner: input.owner,
      kind: 'file',
      title: input.name.trim(),
      url: null,
      mime: input.mime?.trim() || null,
      size,
      sha256: null,
      created_at: new Date().toISOString(),
      removed_at: null,
    }
    mockAttachments.push(item)
    mockAttachmentData.set(item.id, input.data_base64)
    return resolve(item)
  },
  open_attachment: ({ body }) => {
    const item = find((body as AttachmentIdInput).id)
    if (item.url) {
      globalThis.open?.(item.url, '_blank', 'noopener')
      return resolve(null)
    }
    const data = mockAttachmentData.get(item.id) ?? ''
    const bytes = Uint8Array.from(atob(data), (ch) => ch.charCodeAt(0))
    const url = URL.createObjectURL(new Blob([bytes], { type: item.mime ?? 'application/octet-stream' }))
    globalThis.open?.(url, '_blank', 'noopener')
    return resolve(null)
  },
  remove_attachment: ({ body }) => {
    const item = find((body as AttachmentIdInput).id)
    item.removed_at = new Date().toISOString()
    return resolve({ ...item })
  },
  rename_attachment: ({ body }) => {
    const input = body as RenameAttachmentInput
    const item = find(input.id)
    if (!input.title.trim()) throw { code: 'INVALID_INPUT', message: 'a name is required' }
    item.title = input.title.trim()
    return resolve({ ...item })
  },
  // The browser has no file system: «Сохранить как…» downloads the file instead.
  save_attachment_as: ({ body }) => {
    const item = find((body as AttachmentIdInput).id)
    const data = mockAttachmentData.get(item.id) ?? ''
    const link = document.createElement('a')
    link.href = `data:${item.mime ?? 'application/octet-stream'};base64,${data}`
    link.download = item.title
    link.click()
    return resolve(item.title)
  },
  restore_attachment: ({ body }) => {
    const item = find((body as AttachmentIdInput).id, true)
    item.removed_at = null
    return resolve({ ...item })
  },
}
