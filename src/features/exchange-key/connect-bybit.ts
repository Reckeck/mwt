import { call } from '@/shared/api/ipc'

/** Names of the three Bybit accounts in the UI language. */
export interface BybitAccountNames {
  funding: string
  unified: string
  earn: string
}

/**
 * Store a read-only Bybit key for an exchange wallet, then give the wallet the
 * sub-accounts Bybit shows in use (the unified one always). Returns how many
 * sub-accounts the wallet has afterwards.
 */
export async function connectBybit(
  storeId: string,
  apiKey: string,
  apiSecret: string,
  names: BybitAccountNames,
): Promise<number> {
  await call<ExchangeKeyStatus>('set_exchange_key', {
    input: { store_id: storeId, api_key: apiKey, api_secret: apiSecret },
  })
  const ids = await call<string[]>('create_bybit_accounts', {
    input: {
      store_id: storeId,
      funding_name: names.funding,
      unified_name: names.unified,
      earn_name: names.earn,
      detect: true,
    } satisfies CreateBybitAccountsInput,
  })
  return ids.length
}
