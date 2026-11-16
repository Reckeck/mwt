/**
 * Whether the crypto engine drives this wallet: a connected address or an exchange read through its
 * key (`Store.crypto_account_id`, decision P5). Such a wallet only reads: every action that would
 * create, edit or delete an operation, or correct the balance by hand, is off. The static check in
 * `tests/checks/crypto-readonly.check.ts` keeps it that way.
 */
export function isEngineDriven(store: Pick<Store, 'crypto_account_id'> | null | undefined): boolean {
  return Boolean(store?.crypto_account_id)
}
