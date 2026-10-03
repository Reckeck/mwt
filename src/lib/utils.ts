import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** e.g. `0xabc123...` -> short explorer label. */
export function shortHash(h: string | null | undefined, n = 10): string {
  if (!h) return ''
  if (h.length <= n + 2) return h
  return `${h.slice(0, n / 2)}…${h.slice(-n / 2)}`
}