import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** e.g. `0xabc123...` -> short explorer label. */
export function shortHash(hash: string | null | undefined, chars = 10): string {
  if (!hash) return ''
  if (hash.length <= chars + 2) return hash
  return `${hash.slice(0, chars / 2)}…${hash.slice(-chars / 2)}`
}
