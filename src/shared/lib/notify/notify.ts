import { toast } from 'vue-sonner'

import { i18n } from '@/shared/config/i18n'
import { anyErrorMessage } from '@/shared/lib/vault-error'

import type { NotifyOptions } from './notify.types'

/** Same text within this window is shown once: a double click must not stack two toasts. */
const REPEAT_WINDOW_MS = 1500
const recent = new Map<string, number>()

const translate = (key: string): string => i18n.global.t(key)

/** An i18n key becomes its text; anything that is not a known key is shown as it is. */
export function textOf(keyOrText: string): string {
  return i18n.global.te(keyOrText) ? translate(keyOrText) : keyOrText
}

function fresh(kind: string, text: string): boolean {
  const key = `${kind}:${text}`
  const now = Date.now()
  const last = recent.get(key)
  recent.set(key, now)
  return last === undefined || now - last > REPEAT_WINDOW_MS
}

/** The backend's reason, translated by its code (`errors.*`), or its own message. */
export function errorReason(err: unknown): string {
  return anyErrorMessage(err, translate)
}

/** A toast with a button to take the action back stays long enough to reach it. */
const ACTION_TOAST_MS = 10_000
let actionToastUntil = 0

/**
 * Resolves once no toast with a button (`Undo`) is on screen: a notice that comes
 * in meanwhile waits, so it does not push the undo out of reach.
 */
export function afterActionToast(): Promise<void> {
  const wait = actionToastUntil - Date.now()
  return wait > 0 ? new Promise((resolve) => setTimeout(resolve, wait)) : Promise.resolve()
}

function success(keyOrText: string, opts: NotifyOptions = {}): void {
  const text = textOf(keyOrText)
  if (!fresh('success', text)) return
  if (opts.action) actionToastUntil = Date.now() + ACTION_TOAST_MS
  toast.success(text, {
    description: opts.description,
    action: opts.action,
    cancel: opts.cancel,
    duration: opts.action ? ACTION_TOAST_MS : undefined,
  })
}

/**
 * A failure: the title says what did not happen, the description why (and so
 * how to fix it). `err` may be `null` when there is no backend error to read.
 */
function error(err: unknown, titleKey = 'toast.failed', opts: NotifyOptions = {}): void {
  const title = textOf(titleKey)
  const description = opts.description ?? (err === null || err === undefined ? '' : errorReason(err))
  if (!fresh('error', `${title}\n${description}`)) return
  toast.error(title, {
    description: description || undefined,
    action: opts.action,
    cancel: opts.cancel,
    duration: opts.sticky ? Number.POSITIVE_INFINITY : undefined,
  })
}

function warning(keyOrText: string, opts: NotifyOptions = {}): void {
  const text = textOf(keyOrText)
  if (!fresh('warning', text)) return
  toast.warning(text, { description: opts.description, action: opts.action })
}

function info(keyOrText: string, opts: NotifyOptions = {}): void {
  const text = textOf(keyOrText)
  if (!fresh('info', text)) return
  toast.info(text, { description: opts.description, action: opts.action })
}

/**
 * A long action (sync, import, price refresh): a spinner toast that turns into
 * the outcome. Resolves with the promise's value; rethrows its error.
 */
function promise<T>(
  work: Promise<T>,
  keys: { loading: string; success: string | ((data: T) => string); error?: string },
): Promise<T> {
  toast.promise(work, {
    loading: textOf(keys.loading),
    success: (data: T) => (typeof keys.success === 'function' ? keys.success(data) : textOf(keys.success)),
    error: (err: unknown) => `${textOf(keys.error ?? 'toast.failed')}: ${errorReason(err)}`,
  })
  return work
}

let savedTimer: ReturnType<typeof setTimeout> | undefined
/**
 * Settings that save as you change them report once, after the changes settle,
 * instead of a toast per keystroke or per click.
 */
function saved(delayMs = 600): void {
  clearTimeout(savedTimer)
  savedTimer = setTimeout(() => success('ui.saved'), delayMs)
}

export const notify = { success, error, warning, info, promise, saved }
