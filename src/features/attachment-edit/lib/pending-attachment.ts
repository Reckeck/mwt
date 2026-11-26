/** A file or a link picked in a form before the operation exists; it is attached right after the save. */
export type PendingAttachment =
  | { kind: 'file'; name: string; mime: string | null; dataBase64: string }
  | { kind: 'link'; url: string }

/** The core refuses a file over this (25 MB), so the form says so before it is read. */
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024

export const pendingLabel = (item: PendingAttachment) => (item.kind === 'file' ? item.name : item.url)

/** A link is an address with a dot in it, with or without the scheme (`t.me/chat`, `https://…`). */
export function looksLikeLink(text: string): boolean {
  return /^(https?:\/\/)?[^\s/.]+\.[^\s]+$/i.test(text.trim())
}

/** The base64 body of a file (no `data:` header). */
export async function readPendingFile(file: File): Promise<PendingAttachment> {
  const bytes = new Uint8Array(await file.arrayBuffer())
  let binary = ''
  const chunk = 0x8000
  for (let at = 0; at < bytes.length; at += chunk) binary += String.fromCharCode(...bytes.subarray(at, at + chunk))
  return { kind: 'file', name: file.name, mime: file.type || null, dataBase64: btoa(binary) }
}
