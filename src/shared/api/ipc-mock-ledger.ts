/**
 * Browser-mock half of the ledger: transactions, contacts, categories and
 * rules for `pnpm dev`. Mirrors the Rust shapes closely enough to click
 * through the UI; the real rules (V1–V14) live only in the backend.
 */

const GROUPS: Array<[CategoryGroup, string[]]> = [
  [
    'income',
    [
      'salary',
      'advance',
      'bonus',
      'freelance',
      'side_job',
      'gift_received',
      'cashback',
      'deposit_interest',
      'income_other',
    ],
  ],
  [
    'fixed',
    [
      'rent_mortgage',
      'utilities',
      'internet',
      'mobile',
      'loan_payment',
      'loan_interest',
      'installments',
      'taxes',
      'insurance',
      'subscriptions',
    ],
  ],
  [
    'daily',
    [
      'groceries',
      'food_delivery',
      'transport',
      'taxi',
      'fuel',
      'medicine',
      'pharmacy',
      'therapy',
      'sport',
      'education',
      'clothing',
      'beauty',
      'household_chemicals',
      'pets',
      'family_help',
      'gifts',
      'repair',
      'household',
      'entertainment',
      'travel',
      'other',
    ],
  ],
  ['savings', ['savings']],
  ['service', ['trading', 'fee', 'transfer', 'exchange']],
]

/** Same defaults as `default_categories()` in Rust. */
export const mockCategories: Category[] = GROUPS.flatMap(([group, keys]) =>
  keys.map((key) => ({ key, name: null, group, archived: false })),
)

export const mockContacts: Contact[] = [
  {
    id: 'mock-contact-sky',
    name: 'skycapital',
    kind: 'exchanger',
    accounts: [
      {
        id: 'mock-account-usdt',
        group: 'USDT',
        label: 'TRC-20',
        kind: 'crypto_address',
        value: 'TXa1b2c3d4e5',
        currency: 'usdt',
        network: 'tron',
        active: true,
      },
    ],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
  },
  {
    id: 'mock-contact-mom',
    name: 'Мама',
    kind: 'person',
    accounts: [
      {
        id: 'mock-account-card',
        group: null,
        label: 'Карта',
        kind: 'card',
        value: '4276 1234 5678 9012',
        currency: 'rub',
        network: null,
        active: true,
      },
    ],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
  },
]

export const mockRules: CategorizationRule[] = [
  { id: 'mock-rule-taxi', pattern: 'такси', category: 'taxi', active: true, order: 0 },
]

/** Mirrors `allowed_kinds` / `default_kind` in Rust. */
export function mockKindsFor(kind: StoreKind): TransactionKinds {
  const verbs: TxKind[] = ['topup', 'payment', 'transfer', 'convert']
  switch (kind) {
    case 'exchange':
      return { verbs, special: ['trade_spot', 'trade_futures'], default: 'trade_spot' }
    case 'hot_wallet':
    case 'cold_wallet':
      return { verbs, special: ['defi'], default: 'transfer' }
    case 'asset':
      return { verbs: ['topup'], special: ['asset_sell'], default: 'topup' }
    case 'liability':
      return { verbs: ['payment', 'transfer'], special: [], default: 'transfer' }
    default:
      return { verbs, special: [], default: 'payment' }
  }
}

function asset(ident: AssetIdent): AssetRef {
  const symbol = ident.symbol.trim().toUpperCase()
  return {
    symbol,
    name: ident.name,
    coingecko_id: ident.coingecko_id,
    chain: ident.chain,
    contract: ident.contract,
    quote: symbol,
  }
}

const stamp = (usd: string | null): PriceStamp => ({ price_usd: usd, price_base: null, fx_override: null })
const noStamp = stamp(null)

function fee(input: FeeInput | null): Fee | null {
  return input && Number(input.amount) ? { asset: asset(input.asset), amount: input.amount, price: noStamp } : null
}

function meta(common: TxCommonInput, id?: string, origin?: TxOrigin): TxMeta {
  return {
    id: id ?? crypto.randomUUID(),
    timestamp: common.timestamp || new Date().toISOString(),
    comment: common.comment.trim(),
    categories: common.categories.map((key) => key.trim().toLowerCase()).filter(Boolean),
    flow_id: common.flow_id,
    contact_id: common.contact_id,
    origin: origin ?? 'manual',
  }
}

/** The backend's rule that matters most for clicking through the form. */
function check(tx: Transaction) {
  const kind = Object.keys(tx)[0] as TxKind
  const record = (tx as unknown as Record<TxKind, { meta: TxMeta }>)[kind]
  if ((kind === 'topup' || kind === 'payment') && !record.meta.categories.length) {
    throw { code: 'INVALID_INPUT', message: 'choose a category' }
  }
}

/** Like `apply_categorization_rules`: fill empty categories from the first matching rule. */
function categorize(tx: Transaction) {
  const kind = Object.keys(tx)[0] as TxKind
  const record = (tx as unknown as Record<TxKind, { meta: TxMeta }>)[kind]
  if (record.meta.categories.length) return
  const text = record.meta.comment.toLowerCase()
  const rule = [...mockRules]
    .sort((left, right) => left.order - right.order)
    .find((candidate) => candidate.active && text.includes(candidate.pattern.toLowerCase()))
  if (rule) record.meta.categories = [rule.category]
}

/**
 * `TransactionInput::to_transaction` plus the service's autocategorize and
 * category check. `priceOf` stands in for the cached quotes.
 */
export function mockTransactionFrom(
  input: TransactionInput,
  priceOf: (symbol: string) => number,
  keep?: { id: string; origin: TxOrigin },
): Transaction {
  const priced = (symbol: string) => input.common.price_usd ?? (priceOf(symbol) ? String(priceOf(symbol)) : null)
  const base = meta(input.common, keep?.id, keep?.origin)
  let tx: Transaction
  switch (input.type) {
    case 'topup':
    case 'payment': {
      const main = asset(input.asset)
      const shared = {
        meta: base,
        store_id: input.store_id,
        asset: main,
        amount: input.amount,
        fee: fee(input.fee),
        details: input.details,
        price: stamp(priced(main.symbol)),
      }
      tx =
        input.type === 'topup' ? { topup: { ...shared, from: input.from } } : { payment: { ...shared, to: input.to } }
      break
    }
    case 'transfer': {
      const main = asset(input.asset)
      tx = {
        transfer: {
          meta: base,
          from: input.from,
          to: input.to,
          asset: main,
          amount: input.amount,
          fee: fee(input.fee),
          network: input.network,
          tx_hash: input.tx_hash,
          debt: input.debt,
          price: stamp(priced(main.symbol)),
        },
      }
      break
    }
    case 'convert':
      tx = {
        convert: {
          meta: base,
          from: {
            account: input.from.account,
            asset: asset(input.from.asset),
            amount: input.from.amount,
            price: noStamp,
          },
          to: {
            account: input.to.account,
            asset: asset(input.to.asset),
            amount: input.to.amount,
            price: stamp(priced(input.to.asset.symbol)),
          },
          fee: fee(input.fee),
          rate_override: input.rate_override,
        },
      }
      break
    case 'trade_spot':
      tx = {
        trade_spot: {
          meta: base,
          store_id: input.store_id,
          sold: { asset: asset(input.sold.asset), qty: input.sold.qty, price: stamp(priced(input.sold.asset.symbol)) },
          bought: {
            asset: asset(input.bought.asset),
            qty: input.bought.qty,
            price: stamp(priced(input.bought.asset.symbol)),
          },
          fee: fee(input.fee),
          liquidity: input.liquidity,
          order_id: input.order_id,
        },
      }
      break
    case 'trade_futures': {
      const symbol = input.symbol.trim().toUpperCase()
      tx = {
        trade_futures: {
          meta: base,
          store_id: input.store_id,
          contract: {
            symbol,
            name: symbol,
            coingecko_id: null,
            chain: null,
            contract: `perp:${input.venue}`,
            quote: symbol,
          },
          settle: asset(input.settle),
          direction: input.direction,
          action: input.action,
          quantity: input.quantity,
          fill_price: input.fill_price,
          leverage: input.leverage,
          fee: fee(input.fee),
          closed_pnl: input.closed_pnl,
          order_id: input.order_id,
          category: input.category,
          price: stamp('1'),
        },
      }
      break
    }
    case 'defi': {
      const main = asset(input.asset)
      tx = {
        defi: {
          meta: base,
          store_id: input.store_id,
          action: input.action,
          asset: main,
          amount: input.amount,
          protocol: input.protocol,
          pool_id: input.pool_id,
          network: input.network,
          tx_hash: input.tx_hash,
          fee: fee(input.fee),
          price: stamp(priced(main.symbol)),
        },
      }
      break
    }
    case 'asset_sell':
      tx = {
        asset_sell: {
          meta: base,
          store_id: input.store_id,
          asset: asset(input.asset),
          unit_price: input.unit_price,
          quantity: input.quantity,
          currency: asset(input.currency),
          proceeds_to: input.proceeds_to,
          fee: fee(input.fee),
          price: stamp('1'),
        },
      }
      break
  }
  categorize(tx)
  check(tx)
  return tx
}

/** Personal debts per contact and asset, like `contact_balances` in Rust. */
export function mockContactBalances(txs: Transaction[]): ContactBalance[] {
  const out = new Map<string, ContactBalance>()
  for (const tx of txs) {
    if (!('transfer' in tx) || !tx.transfer.debt) continue
    const { from, to, debt, amount } = tx.transfer
    const contact = 'contact' in from ? from.contact.contact_id : 'contact' in to ? to.contact.contact_id : null
    if (!contact) continue
    const key = `${contact}:${tx.transfer.asset.symbol}`
    const row = out.get(key) ?? { contact_id: contact, asset: tx.transfer.asset, receivable: '0', payable: '0' }
    const delta = Number(amount)
    if (debt === 'lend') row.receivable = String(Number(row.receivable) + delta)
    if (debt === 'collect') row.receivable = String(Number(row.receivable) - delta)
    if (debt === 'borrow') row.payable = String(Number(row.payable) + delta)
    if (debt === 'repay') row.payable = String(Number(row.payable) - delta)
    out.set(key, row)
  }
  return [...out.values()].filter((row) => Number(row.receivable) || Number(row.payable))
}

/** Sent and received per asset with one contact, from principal legs. */
export function mockTurnover(
  txs: Transaction[],
  contactId: string,
  accountId: string | null,
  legsOf: (tx: Transaction) => Array<{ asset: AssetRef; amount: string; role: LegRole }>,
): ContactTurnover {
  const rows = new Map<string, TurnoverRow>()
  let count = 0
  for (const tx of txs) {
    const kind = Object.keys(tx)[0] as TxKind
    const record = (tx as unknown as Record<TxKind, { meta: TxMeta; from?: AccountRef; to?: AccountRef }>)[kind]
    const sides = [record.from, record.to].filter((side): side is AccountRef =>
      Boolean(side && typeof side === 'object'),
    )
    const named = sides.some(
      (side) =>
        'contact' in side &&
        side.contact.contact_id === contactId &&
        (!accountId || side.contact.account_id === accountId),
    )
    if (!named && (accountId || record.meta.contact_id !== contactId)) continue
    count += 1
    for (const leg of legsOf(tx).filter((entry) => entry.role === 'principal')) {
      const row = rows.get(leg.asset.symbol) ?? { asset: leg.asset, sent: '0', received: '0' }
      if (leg.amount.startsWith('-')) row.sent = String(Number(row.sent) + Math.abs(Number(leg.amount)))
      else row.received = String(Number(row.received) + Number(leg.amount))
      rows.set(leg.asset.symbol, row)
    }
  }
  return {
    contact_id: contactId,
    account_id: accountId,
    base_currency: 'usd',
    rows: [...rows.values()],
    sent_base: '0',
    received_base: '0',
    tx_count: count,
  }
}
