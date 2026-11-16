import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import { isHttps, openLink, openOperationInBrowser } from './open-in-browser'

vi.mock('@/shared/api/ipc', () => ({ call: vi.fn() }))
const mocked = vi.mocked(call)

describe('opening an explorer page', () => {
  beforeEach(() => mocked.mockReset())

  it('only an https address is ever opened', async () => {
    for (const url of [
      'http://etherscan.io/tx/0x1',
      'javascript:alert(1)',
      'data:text/html,x',
      '//etherscan.io',
      '',
      undefined,
      null,
    ]) {
      expect(isHttps(url)).toBe(false)
      expect(await openLink(url)).toBe(false)
    }
    expect(mocked).not.toHaveBeenCalled()
    expect(await openLink('https://etherscan.io/tx/0x1')).toBe(true)
    expect(mocked).toHaveBeenCalledWith('open_explorer', { url: 'https://etherscan.io/tx/0x1' })
  })

  it('asks the vault for the address at the click, and reaches the explorer only through open_explorer', async () => {
    mocked.mockImplementation(async (cmd: string) =>
      cmd === 'crypto_operation_detail' ? { url: 'https://etherscan.io/tx/0x1' } : {},
    )
    await openOperationInBrowser('cev:1')
    expect(mocked.mock.calls.map(([cmd]) => cmd)).toEqual(['crypto_operation_detail', 'open_explorer'])
  })

  it('a card without an https address opens nothing', async () => {
    mocked.mockResolvedValue({ url: 'http://insecure.example/tx/1' })
    expect(await openOperationInBrowser('cev:1')).toBe(false)
    expect(mocked.mock.calls.map(([cmd]) => cmd)).toEqual(['crypto_operation_detail'])
  })
})
