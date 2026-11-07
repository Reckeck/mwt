import { call } from './ipc'

/**
 * Open a plain webview file input (file chooser or camera). The system picker takes the app to the
 * background, where the Android autolock would close the vault and lose the form, so the backend is
 * told to hold the lock for a few minutes first.
 *
 * `click()` on a file input needs a user gesture: it runs synchronously, the hold is only sent
 * before it and never awaited.
 */
export function openSystemPicker(input: HTMLInputElement): void {
  // A failed hold only means the old behavior (the lock may fire), the picker must still open.
  call('autolock_hold_for_picker').catch(() => undefined)
  input.click()
}
