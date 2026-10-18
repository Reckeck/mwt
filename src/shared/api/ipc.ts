/// Central typed bridge to the Rust backend.
///
/// Inside Tauri uses `@tauri-apps/api/core`; in a plain browser (hot `pnpm dev`
/// without the desktop shell) falls back to a small in-memory mock so the UI
/// can be iterated on.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { listenMock } from './mock-bus'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export interface AppError {
  code: string
  message: string
}

export function parseError(raw: unknown): AppError {
  // `call` already throws the parsed shape (and the mock throws it directly).
  if (raw && typeof raw === 'object' && 'code' in raw && 'message' in raw) {
    const { code, message } = raw as Record<string, unknown>
    if (typeof code === 'string' && typeof message === 'string') return { code, message }
  }
  const text = typeof raw === 'string' ? raw : String(raw ?? 'Unknown error')
  const sep = text.indexOf(': ')
  if (sep > 0 && text.slice(0, sep) === text.slice(0, sep).toUpperCase()) {
    return { code: text.slice(0, sep), message: text.slice(sep + 2) }
  }
  return { code: 'UNKNOWN', message: text }
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) {
    const { mock } = await import('./mock')
    return mock(cmd, args) as Promise<T>
  }
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    throw parseError(err)
  }
}

export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  // Browser dev mode: the mock's own events (`mock-bus`).
  if (!isTauri) return listenMock(event, handler as (payload: unknown) => void)
  return listen<T>(event, (received) => handler(received.payload))
}
