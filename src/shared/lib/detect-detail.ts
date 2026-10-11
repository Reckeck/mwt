/** What a pasted requisite looks like: its kind, a network for crypto, the value cleaned up. */
export interface DetectedDetail {
  kind: DetailKind
  network: Chain | null
  value: string
}

const BASE58 = '[1-9A-HJ-NP-Za-km-z]'

/** Luhn checksum: tells a card number from any other long digit string. */
function luhn(digits: string): boolean {
  let sum = 0
  let double = false
  for (let index = digits.length - 1; index >= 0; index -= 1) {
    let digit = Number(digits[index])
    if (double) {
      digit *= 2
      if (digit > 9) digit -= 9
    }
    sum += digit
    double = !double
  }
  return sum % 10 === 0
}

/** Crypto address formats, most specific first: an EVM address could not be anything else. */
const CRYPTO: { pattern: RegExp; network: Chain }[] = [
  { pattern: /^0x[0-9a-fA-F]{40}$/, network: 'ethereum' },
  { pattern: /^bc1[02-9ac-hj-np-z]{11,71}$/, network: 'bitcoin' },
  { pattern: new RegExp(`^[13]${BASE58}{25,34}$`), network: 'bitcoin' },
  { pattern: new RegExp(`^T${BASE58}{33}$`), network: 'tron' },
  { pattern: /^(ltc1[02-9ac-hj-np-z]{11,71})$/, network: 'litecoin' },
  { pattern: new RegExp(`^[LM]${BASE58}{26,33}$`), network: 'litecoin' },
  { pattern: new RegExp(`^D${BASE58}{33}$`), network: 'dogecoin' },
  { pattern: new RegExp(`^r${BASE58}{24,34}$`), network: 'xrp' },
  { pattern: /^(EQ|UQ)[A-Za-z0-9_-]{46}$/, network: 'ton' },
  { pattern: /^addr1[02-9ac-hj-np-z]{50,120}$/, network: 'cardano' },
  { pattern: /^cosmos1[02-9ac-hj-np-z]{38}$/, network: 'cosmos' },
  { pattern: new RegExp(`^${BASE58}{32,44}$`), network: 'solana' },
]

/**
 * Guess what a pasted requisite is. Never throws: an unknown string is
 * `other`, so the user can always correct the guess by hand.
 */
export function detectDetail(raw: string): DetectedDetail {
  const value = raw.trim()
  const compact = value.replace(/[\s-]/g, '')
  if (!value) return { kind: 'other', network: null, value }

  // A link, or a bare domain with a letter TLD («t.me/name», «bank.com»); crypto addresses have no dots.
  if (/^https?:\/\//i.test(value) || /^[\w-]+(\.[\w-]+)*\.[a-z]{2,}(\/\S*)?$/i.test(value)) {
    return { kind: 'url', network: null, value }
  }
  if (/^\+\d[\d\s()-]{8,}$/.test(value)) {
    return { kind: 'phone', network: null, value: `+${value.replace(/\D/g, '')}` }
  }
  if (/^[A-Z]{2}\d{2}[A-Z0-9]{10,30}$/i.test(compact)) {
    return { kind: 'iban', network: null, value: compact.toUpperCase() }
  }
  if (/^\d{13,19}$/.test(compact) && luhn(compact)) {
    return { kind: 'card', network: null, value: compact.replace(/(\d{4})(?=\d)/g, '$1 ') }
  }
  if (/^\d{8,30}$/.test(compact)) return { kind: 'account', network: null, value: compact }
  for (const entry of CRYPTO) {
    if (entry.pattern.test(value)) return { kind: 'crypto_address', network: entry.network, value }
  }
  return { kind: 'other', network: null, value }
}
