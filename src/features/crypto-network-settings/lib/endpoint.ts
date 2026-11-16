import { i18n } from '@/shared/config/i18n'
import type { FieldError } from '@/shared/lib/form'

/**
 * The address rules of `service/cryptotrack/network.rs`, checked here too so the message stands under the field
 * on save (rule 13). Rust checks again: this is the courtesy, not the guard.
 *
 * Only `https`; the one exception is `http` to localhost for the user's own node. An empty field means the preset.
 */
export type EndpointKind = 'rpc' | 'blockscout' | 'explorer'

const message = (key: string) => i18n.global.t(`cryptoTrack.network.err.${key}`)

const isOwnNode = (host: string) => host === 'localhost' || host === '127.0.0.1' || host === '[::1]'

export function validateEndpoint(kind: EndpointKind, raw: string): FieldError {
  const text = raw.trim()
  if (!text) return null
  if (/[\s\\]/.test(text)) return message('whitespace')
  let url: URL
  try {
    url = new URL(text)
  } catch {
    return message('invalid')
  }
  const https = url.protocol === 'https:'
  // An explorer is only a link that gets opened: never http, and a real host name.
  const ownNode = kind !== 'explorer' && url.protocol === 'http:' && isOwnNode(url.hostname)
  if (!https && !ownNode) return message(kind === 'explorer' ? 'scheme' : 'schemeLocal')
  if (url.username || url.password) return message('credentials')
  if (url.hash) return message('fragment')
  if (kind !== 'rpc' && url.search) return message('query')
  if (kind === 'explorer' && !url.hostname.includes('.')) return message('host')
  return null
}

/** The optional Blockscout key: what an `Authorization` header can carry. Empty is fine (keyless). */
export function validateKey(raw: string): FieldError {
  const text = raw.trim()
  if (!text) return null
  return /^[\x21-\x7e]{1,256}$/.test(text) ? null : message('key')
}
