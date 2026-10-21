/** Default categories, demo contacts, rules and the kinds each wallet allows. */

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
      'dividends',
      'coupons',
      'staking',
      'rent_income',
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
  ['service', ['trading', 'fee', 'transfer', 'exchange', 'opening_balance']],
]

/** Same defaults as `default_categories()` in Rust. */
export const mockCategories: Category[] = GROUPS.flatMap(([group, keys]) =>
  keys.map((key) => ({ key, name: null, group, archived: false, essential: null, passive: null })),
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
    group: 'P2P',
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
    group: '',
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
    case 'crypto':
      return { verbs, special: ['defi'], default: 'transfer' }
    case 'property':
      return { verbs: ['topup'], special: ['asset_sell'], default: 'topup' }
    case 'liability':
      return { verbs: ['payment', 'transfer'], special: [], default: 'transfer' }
    default:
      return { verbs, special: [], default: 'payment' }
  }
}
