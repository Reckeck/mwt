/** Wallet kinds, presets, envelopes and exchange defaults. */

import { mockOwnPositions } from './tree'

/** What an exchange import brings in, as Rust's `ImportSettings::default()`. */
export function mockImportSettings(): ImportSettings {
  return {
    trades: true,
    transfers: true,
    earn: true,
    auto_confirm: false,
    auto_reconcile: true,
    coins: [],
    since: null,
    categories: { trades: null, transfers: null, earn: null },
    accounts: [],
  }
}

export const mockEnvelopes: Envelope[] = []
export const mockDebtTerms: DebtTerms[] = []

export function mockEnvelopeView(envelope: Envelope): EnvelopeView {
  const held = mockOwnPositions(envelope.store_id)
    .filter((position) => position.asset.symbol.toUpperCase() === envelope.asset.symbol.toUpperCase())
    .reduce((sum, position) => sum + Number(position.quantity), 0)
  const earmarked = mockEnvelopes
    .filter((entry) => entry.store_id === envelope.store_id && entry.asset.symbol === envelope.asset.symbol)
    .reduce((sum, entry) => sum + Number(entry.allocated), 0)
  const free = held - earmarked
  const target = Number(envelope.target ?? 0)
  return {
    envelope,
    free: String(free),
    pct: target ? (Number(envelope.allocated) / target) * 100 : null,
    short: free < 0,
  }
}

/** Mirrors `AutomationPreset::tx_kind` / `category` in Rust. */
function mockPresets(names: AutomationPreset[]): PresetInfo[] {
  const kinds: Record<AutomationPreset, TxKind> = {
    expense: 'payment',
    top_up: 'transfer',
    salary: 'topup',
    card_payment: 'payment',
    to_savings: 'transfer',
    deposit_interest: 'topup',
    dca_buy: 'convert',
    staking_reward: 'topup',
    market_transfer: 'transfer',
    rent_income: 'topup',
    property_tax: 'payment',
    insurance: 'payment',
    utilities: 'payment',
    loan_payment: 'transfer',
    vehicle_service: 'payment',
    fuel: 'payment',
  }
  const categories: Partial<Record<AutomationPreset, string>> = {
    salary: 'salary',
    deposit_interest: 'deposit_interest',
    staking_reward: 'income_other',
    rent_income: 'income_other',
    to_savings: 'savings',
    dca_buy: 'trading',
    market_transfer: 'transfer',
    top_up: 'transfer',
    loan_payment: 'loan_payment',
    property_tax: 'taxes',
    insurance: 'insurance',
    utilities: 'utilities',
    vehicle_service: 'repair',
    fuel: 'fuel',
  }
  return names.map((preset) => ({ preset, tx_kind: kinds[preset], category: categories[preset] ?? null }))
}

/** Mirrors `WalletCatalog::build` in Rust. */
export function mockWalletCatalog(): WalletCatalog {
  const sub = (sub_type: SubType, shape: BindingShapeName, purpose: Purpose | null = null): SubTypeInfo => ({
    sub_type,
    shape,
    default_purpose: purpose,
    credit: sub_type === 'credit_card',
  })
  return {
    kinds: [
      {
        kind: 'cash',
        presets: mockPresets(['expense', 'top_up']),
        max_roots: 1,
        user_creatable: false,
        crypto: false,
        default_purpose: 'everyday',
        sub_types: [sub('pocket', 'asset', 'everyday')],
      },
      {
        kind: 'bank',
        presets: mockPresets(['salary', 'card_payment', 'to_savings', 'deposit_interest']),
        max_roots: null,
        user_creatable: true,
        crypto: false,
        default_purpose: 'everyday',
        sub_types: [
          sub('card', 'asset', 'everyday'),
          sub('account', 'asset', 'everyday'),
          sub('deposit', 'asset', 'savings'),
          sub('savings', 'asset', 'savings'),
          sub('pds', 'asset', 'savings'),
          sub('credit_card', 'asset', 'everyday'),
        ],
      },
      {
        kind: 'crypto',
        presets: mockPresets(['dca_buy', 'staking_reward']),
        max_roots: null,
        user_creatable: true,
        crypto: true,
        default_purpose: 'investment',
        sub_types: [sub('token', 'asset')],
      },
      {
        kind: 'exchange',
        presets: mockPresets(['dca_buy', 'market_transfer']),
        max_roots: null,
        user_creatable: true,
        crypto: true,
        default_purpose: 'investment',
        sub_types: [
          sub('funding', 'market'),
          sub('unified', 'market'),
          sub('earn', 'market'),
          sub('spot', 'market'),
          sub('futures', 'market'),
          sub('pair', 'pair'),
        ],
      },
      {
        kind: 'property',
        presets: mockPresets(['rent_income', 'property_tax', 'insurance', 'utilities']),
        max_roots: null,
        user_creatable: true,
        crypto: false,
        default_purpose: 'investment',
        sub_types: [],
      },
      {
        kind: 'loyalty',
        presets: [],
        max_roots: null,
        user_creatable: true,
        crypto: false,
        default_purpose: 'savings',
        sub_types: [],
      },
      {
        kind: 'liability',
        presets: mockPresets(['loan_payment']),
        max_roots: null,
        user_creatable: true,
        crypto: false,
        default_purpose: 'investment',
        sub_types: [],
      },
    ],
    property_presets: [
      { category: 'flat', presets: mockPresets(['rent_income', 'insurance', 'utilities']) },
      { category: 'house', presets: mockPresets(['rent_income', 'insurance', 'utilities']) },
      { category: 'car', presets: mockPresets(['insurance', 'vehicle_service', 'fuel']) },
      { category: 'land', presets: mockPresets(['property_tax']) },
      { category: 'other', presets: mockPresets(['expense']) },
    ],
    providers: [
      { provider: 'bybit', supported: true },
      { provider: 'binance', supported: false },
      { provider: 'okx', supported: false },
    ],
  }
}

export function mockTerminal(): TerminalState {
  return {
    watchlist: ['BTCUSDT', 'ETHUSDT'],
    active_symbol: null,
    timeframe: '1h',
    indicators: [],
    drawings: {},
    layout: { show_watchlist: true, watchlist_width: 240 },
  }
}
