import { notify } from '@/shared/lib/notify'

/**
 * Copy text and say so; a refused clipboard says how to get it anyway.
 * `okKey` names what was copied when «Скопировано» is not enough.
 */
export async function copyText(text: string, okKey = 'ui.copied'): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    notify.success(okKey)
    return true
  } catch (err) {
    notify.error(err, 'ui.copyFailed', { description: '' })
    return false
  }
}
