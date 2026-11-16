import { call } from '@/shared/api/ipc'

/** Only an `https` address is ever opened (the backend builds them so; this is the second lock). */
export const isHttps = (url: string | null | undefined): url is string =>
  typeof url === 'string' && /^https:\/\//i.test(url)

/**
 * Open the explorer page of an operation in the system browser, through `open_explorer` — the mechanism
 * every explorer link already uses. Called only from a click: the address is asked from the vault at that
 * moment (the list never carries one) and nothing is requested from the explorer before it opens.
 */
export async function openOperationInBrowser(id: string): Promise<boolean> {
  const detail = await call<CryptoOperationDetail>('crypto_operation_detail', { input: { id, with_candidates: false } })
  return openLink(detail.url)
}

/** Open an address already in hand (the card has its own); refuses anything but `https`. */
export async function openLink(url: string | null | undefined): Promise<boolean> {
  if (!isHttps(url)) return false
  await call('open_explorer', { url })
  return true
}
