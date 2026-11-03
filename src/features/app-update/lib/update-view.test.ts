import { describe, expect, it } from 'vitest'

import { describeUpdate, noticeVersion } from './update-view'

const offer = (patch: Partial<UpdateOffer>): UpdateOffer => ({
  version: '1.2.0',
  notes: 'Notes',
  release_url: 'https://example.test/release',
  download_url: 'https://example.test/app.apk',
  can_self_update: false,
  hint: 'release_page',
  ...patch,
})
const status = (phase: UpdatePhase): UpdateStatusDto => ({
  phase,
  current_version: '1.0.0',
  install_kind: 'unknown',
  channel: 'prod',
  auto_update: true,
  last_check: null,
})

describe('describeUpdate', () => {
  it('is idle without a status', () => {
    expect(describeUpdate(undefined).kind).toBe('idle')
  })
  it('clamps download percent', () => {
    expect(describeUpdate(status({ type: 'downloading', percent: 140.4 })).percent).toBe(100)
  })
  it('offers a restart for a ready update', () => {
    const view = describeUpdate(status({ type: 'ready', version: '1.2.0' }))
    expect(view).toMatchObject({ kind: 'ready', version: '1.2.0', action: 'restart' })
    expect(noticeVersion(view)).toBe('1.2.0')
  })
  it('maps the manual hints to actions', () => {
    expect(describeUpdate(status({ type: 'available', offer: offer({}) }))).toMatchObject({
      action: 'openRelease',
      url: 'https://example.test/release',
    })
    expect(describeUpdate(status({ type: 'available', offer: offer({ hint: 'apk' }) }))).toMatchObject({
      action: 'openApk',
      url: 'https://example.test/app.apk',
    })
    expect(describeUpdate(status({ type: 'available', offer: offer({ hint: 'package_manager' }) }))).toMatchObject({
      action: 'packageManager',
      url: null,
    })
    expect(describeUpdate(status({ type: 'available', offer: offer({ hint: 'none' }) })).action).toBe('none')
  })
  it('keeps the error text and gives no notice', () => {
    const view = describeUpdate(status({ type: 'error', message: 'offline' }))
    expect(view).toMatchObject({ kind: 'error', message: 'offline' })
    expect(noticeVersion(view)).toBeNull()
  })
})
