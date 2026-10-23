import { call } from '@/shared/api/ipc'

import type { StagedAttachment } from '../attachment.types'
import { fileToBase64 } from '../lib/read-file'

/**
 * Attaches what a form held to the entity it has just saved, one by one.
 * Returns the names that did not make it: the entity is saved either way.
 */
export async function attachStaged(owner: EntityRef, staged: readonly StagedAttachment[]): Promise<string[]> {
  const missed: string[] = []
  for (const item of staged) {
    try {
      if (item.kind === 'link') {
        await call<Attachment>('add_attachment_link', { input: { owner, url: item.url, title: item.title } })
      } else {
        const data = await fileToBase64(item.file)
        await call<Attachment>('add_attachment_file', {
          input: { owner, name: item.name, mime: item.file.type || null, data_base64: data },
        })
      }
    } catch {
      missed.push(item.kind === 'link' ? item.title || item.url : item.name)
    }
  }
  return missed
}
