/** A file's bytes as base64, the way `add_attachment_file` takes them. */
export async function fileToBase64(file: Blob): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer())
  let binary = ''
  // `String.fromCharCode(...bytes)` overflows the stack on a few megabytes; chunks do not.
  for (let start = 0; start < bytes.length; start += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(start, start + 0x8000))
  }
  return btoa(binary)
}

/** «t.me/x», «https://a.b/c», «mailto:…»: text that reads as one address, nothing else. */
export function looksLikeLink(text: string): boolean {
  const value = text.trim()
  if (!value || /\s/.test(value)) return false
  if (/^mailto:/i.test(value)) return value.includes('@')
  return /^(https?:\/\/)?[^\s/]+\.[^\s/]{2,}(\/\S*)?$/i.test(value)
}
