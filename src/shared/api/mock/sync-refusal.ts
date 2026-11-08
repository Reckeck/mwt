/**
 * Lets a test (or a console) produce the `SECURITY_LEVEL_REQUIRED` sync refusal the way the backend does:
 * the same `sync-error` event reaches both sides, the frontend tells them apart by this device's level.
 */
import { emitMock } from '../mock-bus'

export function emitMockLevelRefusal(peerName: string | null) {
  const event: SyncErrorEvent = {
    code: 'SECURITY_LEVEL_REQUIRED',
    message: 'SECURITY_LEVEL_REQUIRED: crypto data needs security level 3',
    peer_name: peerName,
    peer_version: null,
    own_version: null,
  }
  emitMock('sync-error', event)
}
