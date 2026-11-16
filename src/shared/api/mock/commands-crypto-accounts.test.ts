import { describe, expect, it } from 'vitest'

import { cryptoAccountsCommands } from './commands-crypto-accounts'
import { mockStores } from './demo'

const SEED = 'legal winner thank year wave sausage worth useful legal winner thank yellow'
const KEY = '0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318'
const connect = (address: string, name = 'Main') =>
  cryptoAccountsCommands.crypto_connect_address({
    cmd: 'crypto_connect_address',
    args: undefined,
    body: { chain: 'ethereum', address, name, start_date: '2026-01-01' },
  })

describe('the crypto accounts mock', () => {
  it('refuses a seed phrase and a private key without storing or echoing them', async () => {
    const stores = mockStores.length
    for (const secret of [SEED, KEY]) {
      const refusal = await connect(secret).then(
        () => null,
        (error: { code: string; message: string }) => error,
      )
      expect(refusal?.code).toBe('INVALID_INPUT')
      expect(refusal?.message).toContain('seed phrase or a private key')
      expect(JSON.stringify(refusal)).not.toContain(secret === SEED ? 'winner' : KEY.slice(2, 20))
    }
    expect(mockStores.length).toBe(stores)
    const listed = (await cryptoAccountsCommands.crypto_list_accounts({
      cmd: 'x',
      args: undefined,
      body: undefined,
    })) as unknown[]
    expect(listed).toEqual([])
  })

  it('connects a public address and binds a wallet to it', async () => {
    const done = (await connect('0x7a3F19c4D2b85E6F0a9C41b7cE3a0D8e2f4b9bC1')) as {
      account_id: string
      store_id: string
    }
    const store = mockStores.find((entry) => entry.id === done.store_id)
    expect(store?.kind).toBe('crypto')
    expect(store?.crypto_account_id).toBe(done.account_id)
    const again = await connect('0x7A3F19C4D2B85E6F0A9C41B7CE3A0D8E2F4B9BC1', 'Other').catch(
      (error: { code: string }) => error,
    )
    expect(again).toMatchObject({ code: 'ALREADY_EXISTS' })
  })
})
