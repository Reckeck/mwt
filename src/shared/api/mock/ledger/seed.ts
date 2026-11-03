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
        group: 'Мой банк',
        label: 'Карта Тинькофф',
        kind: 'card',
        value: '5536 9137 8812 4417',
        currency: 'rub',
        network: null,
        active: true,
      },
      {
        id: 'mock-account-mom-phone',
        group: null,
        label: 'Телефон для СБП',
        kind: 'phone',
        value: '+7 916 555 12 34',
        currency: 'rub',
        network: null,
        active: true,
      },
      {
        id: 'mock-account-mom-usdt',
        group: null,
        label: 'USDT TRC-20',
        kind: 'crypto_address',
        value: 'TQ7xkN3vWm8pLr2sYb5cHd9eFg4hJk6F2s9fKp',
        currency: 'usdt',
        network: 'tron',
        active: true,
      },
      {
        id: 'mock-account-mom-sber',
        group: null,
        label: 'Старая карта Сбер',
        kind: 'card',
        value: '4276 3800 1234 0912',
        currency: 'rub',
        network: null,
        active: false,
      },
    ],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
    group: 'Семья',
  },
  {
    id: 'mock-contact-sergey',
    name: 'Сергей',
    kind: 'person',
    accounts: [],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
    group: '',
  },
  {
    id: 'mock-contact-oleg',
    name: 'Олег',
    kind: 'person',
    accounts: [
      {
        id: 'mock-account-oleg-phone',
        group: 'Банки',
        label: 'Телефон (СБП)',
        kind: 'other',
        value: '8 (912) 345-67-89',
        currency: 'rub',
        network: null,
        active: true,
      },
      {
        id: 'mock-account-oleg-card',
        group: 'Банки',
        label: 'Карта Альфа',
        kind: 'card',
        value: '5559 4900 1122 3344',
        currency: 'rub',
        network: null,
        active: true,
      },
    ],
    note: 'Сосед по даче',
    archived: false,
    created_at: new Date().toISOString(),
    group: 'Друзья',
  },
  {
    id: 'mock-contact-rassvet',
    name: 'ООО «Рассвет»',
    kind: 'company',
    accounts: [
      {
        id: 'mock-account-rassvet-acc',
        group: null,
        label: 'Расчётный счёт',
        kind: 'account',
        value: '40702810900000012345',
        currency: 'rub',
        network: null,
        active: true,
      },
      {
        id: 'mock-account-rassvet-iban',
        group: null,
        label: 'Счёт в USD',
        kind: 'iban',
        value: 'DE89370400440532013000',
        currency: 'usd',
        network: null,
        active: true,
      },
    ],
    note: 'Подрядчик по ремонту',
    archived: false,
    created_at: new Date().toISOString(),
    group: '',
  },
  {
    id: 'mock-contact-lena',
    name: 'Лена',
    kind: 'person',
    accounts: [
      {
        id: 'mock-account-lena-phone',
        group: null,
        label: 'Телефон',
        kind: 'phone',
        value: '+7 903 111 22 33',
        currency: 'rub',
        network: null,
        active: true,
      },
    ],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
    group: 'Семья',
  },
  {
    id: 'mock-contact-dima',
    name: 'Дима',
    kind: 'person',
    accounts: [
      {
        id: 'mock-account-dima-card',
        group: null,
        label: 'Старая карта',
        kind: 'card',
        value: '4276 1234 5678 9012',
        currency: 'rub',
        network: null,
        active: false,
      },
    ],
    note: 'Переехал, больше не пересекаемся',
    archived: true,
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
      return { verbs, special: ['trade_spot', 'trade_futures', 'defi'], default: 'trade_spot' }
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
