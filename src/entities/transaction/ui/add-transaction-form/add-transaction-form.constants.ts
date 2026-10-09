import type { TxTag } from '@/shared/lib/transactions'

/** Free-text fields a kind carries beyond the common ones, named as in its input DTO. */
export type KindTextField =
  | 'payee'
  | 'receipt_ref'
  | 'counterparty_name'
  | 'counterparty_account'
  | 'reference'
  | 'merchant'
  | 'card_last4'
  | 'mcc'
  | 'to_address'
  | 'from_address'
  | 'memo'
  | 'protocol'
  | 'pool_id'
  | 'location'
  | 'counterparty'
  | 'market'
  | 'order_id'
  | 'liquidity'

/** Which free-text fields each kind shows, in display order. */
export const KIND_TEXT_FIELDS: Partial<Record<TxTag, KindTextField[]>> = {
  payment: ['payee', 'receipt_ref'],
  bank_transfer: ['counterparty_name', 'counterparty_account', 'reference'],
  card_payment: ['merchant', 'card_last4', 'mcc'],
  wallet_transfer: ['to_address', 'from_address', 'memo'],
  defi_action: ['protocol', 'pool_id'],
  cash_flow: ['location', 'counterparty'],
  exchange_trade: ['market', 'order_id', 'liquidity'],
}

/** Fields the user must fill when the kind shows them: a payment always has a payee. */
export const REQUIRED_TEXT_FIELDS: KindTextField[] = ['payee']

/** Label key per field — the same wording the history uses for the stored value. */
export const TEXT_FIELD_LABELS: Record<KindTextField, string> = {
  payee: 'tx.detail.payee',
  receipt_ref: 'tx.detail.reference',
  counterparty_name: 'tx.detail.counterparty',
  counterparty_account: 'tx.detail.account',
  reference: 'tx.detail.reference',
  merchant: 'tx.detail.merchant',
  card_last4: 'tx.detail.card',
  mcc: 'tx.detail.mcc',
  to_address: 'tx.detail.to',
  from_address: 'tx.detail.from',
  memo: 'tx.detail.memo',
  protocol: 'tx.detail.protocol',
  pool_id: 'tx.detail.pool',
  location: 'tx.detail.location',
  counterparty: 'tx.detail.counterparty',
  market: 'tx.detail.market',
  order_id: 'tx.detail.order',
  liquidity: 'tx.detail.liquidity',
}

export const INCOME_SOURCES: IncomeSource[] = ['other', 'salary', 'reward', 'airdrop', 'staking', 'interest', 'mining']

export const BANK_RAILS: BankRail[] = ['internal', 'sepa', 'swift', 'wire', 'card', 'cashier']

export const DEFI_ACTIONS: DefiAction[] = ['stake', 'unstake', 'claim', 'add_liquidity', 'remove_liquidity']
