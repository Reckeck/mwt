import { describe, expect, it } from 'vitest'

import { syncErrorDescription } from './sync-error-text'

const t = (key: string, params?: Record<string, unknown>) => `${key}|${JSON.stringify(params ?? {})}`

const event = (patch: Partial<SyncErrorEvent>): SyncErrorEvent => ({
  code: 'SYNC_VERSION_MISMATCH',
  message: '',
  peer_name: 'Phone',
  peer_version: '0.9.3',
  own_version: '1.0.0',
  ...patch,
})

describe('syncErrorDescription', () => {
  it('names the device and both versions on a version mismatch', () => {
    expect(syncErrorDescription(event({}), t)).toBe(
      'errors.SYNC_VERSION_MISMATCH_NAMED|{"name":"Phone","version":"0.9.3","own":"1.0.0"}',
    )
  })

  it('says «old version» when the peer sent none', () => {
    expect(syncErrorDescription(event({ peer_version: '' }), t)).toBe(
      'errors.SYNC_VERSION_MISMATCH_OLD|{"name":"Phone","own":"1.0.0"}',
    )
  })

  it('falls back to the plain error without a name', () => {
    expect(syncErrorDescription(event({ peer_name: null }), t)).toBeUndefined()
  })

  it('keeps the cut-off sentences', () => {
    expect(syncErrorDescription(event({ code: 'SYNC_PAUSED_BY_PEER' }), t)).toBe(
      'errors.SYNC_PAUSED_BY_PEER|{"name":"Phone"}',
    )
  })

  it('leaves other codes to the default text', () => {
    expect(syncErrorDescription(event({ code: 'SYNC_CHANNEL' }), t)).toBeUndefined()
  })
})
