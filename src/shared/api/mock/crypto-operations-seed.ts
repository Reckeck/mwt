/**
 * The demo operations of the crypto module: eight by hand (two of them the module's own entries, which have
 * no hash and no page in an explorer) and a deterministic bulk, ten thousand in all (spec E10).
 */

export const USDT = '0xdac17f958d2ee523a2206206994597c13d831ec7'
export const ACCOUNT = 'mock-crypto-main'
const HASH = (seed: number) => `0x${seed.toString(16).padStart(8, '0').repeat(8)}`

export interface MockEvent {
  id: string
  time: string
  kind: CryptoTxKind
  direction: 'in' | 'out'
  symbol: string
  assetKey: string
  amount: string
  from: string
  to: string
  protocol?: string
  gas?: string
  exchange?: string
  hash: string
  /** The module's own entry: it has no hash and no page in an explorer. */
  own?: 'opening' | 'reconciliation'
}
export interface MockFix {
  kind?: CryptoTxKind
  comment?: string
  categories: string[]
  contactId?: string
  flowId?: string
  hidden: boolean
  /** The user answered that this one is not an exchange. */
  notExchange?: boolean
}
/** What the user decided for everything that goes through one contract. */
export interface MockRule {
  kind?: CryptoTxKind
  categories: string[]
  contactId?: string
}

export const CONTACTS: Record<string, string> = { 'mock-contact-coinrush': 'Обменник CoinRush' }
export const SENDER = '0x9f3c000000000000000000000000000000001a71'
export const FIAT = [
  {
    id: 'mock-fiat-tinkoff',
    place: 'Тинькофф',
    amount: '-50000',
    currency: 'RUB',
    time: '2026-11-11T09:00:00Z',
    comment: 'Обменнику',
  },
  {
    id: 'mock-fiat-sber',
    place: 'Сбербанк',
    amount: '-12300',
    currency: 'RUB',
    time: '2026-11-11T08:10:00Z',
    comment: 'Продукты',
  },
  {
    id: 'mock-fiat-cash',
    place: 'Наличные',
    amount: '-50000',
    currency: 'RUB',
    time: '2026-11-10T20:00:00Z',
    comment: '',
  },
]

export function seed(): MockEvent[] {
  const head: MockEvent[] = [
    {
      id: 'copen:mock-crypto-main:ethereum|native:2026-01-01',
      time: '2026-01-01T00:00:00Z',
      kind: 'unknown',
      direction: 'in',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0.35',
      from: '',
      to: '0xmain',
      hash: '',
      own: 'opening',
    },
    {
      id: 'crec:mock-crypto-main:ethereum|native:2026-11-08',
      time: '2026-11-08T09:00:00Z',
      kind: 'unknown',
      direction: 'out',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0.004',
      from: '0xmain',
      to: '',
      hash: '',
      own: 'reconciliation',
    },
    {
      id: 'cev:1',
      time: '2026-11-11T14:20:00Z',
      kind: 'transfer',
      direction: 'in',
      symbol: 'USDT',
      assetKey: `ethereum|${USDT}`,
      amount: '500',
      from: SENDER,
      to: '0xmain',
      hash: HASH(1),
      gas: '0.0011',
    },
    {
      id: 'cev:2',
      time: '2026-11-10T10:00:00Z',
      kind: 'transfer',
      direction: 'out',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0.042',
      from: '0xmain',
      to: '0x4b21000000000000000000000000000000000fc90',
      hash: HASH(2),
      gas: '0.0012',
    },
    {
      id: 'cev:3',
      time: '2026-11-09T12:00:00Z',
      kind: 'transfer',
      direction: 'in',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0.25',
      from: '0x2ad8000000000000000000000000000000004b02',
      to: '0xmain',
      hash: HASH(3),
    },
    {
      id: 'cev:4',
      time: '2026-11-08T09:00:00Z',
      kind: 'approve',
      direction: 'out',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0',
      from: '0xmain',
      to: USDT,
      hash: HASH(4),
      gas: '0.0003',
    },
    {
      id: 'cev:5',
      time: '2026-11-07T09:00:00Z',
      kind: 'swap',
      direction: 'out',
      symbol: 'ETH',
      assetKey: 'ethereum|native',
      amount: '0.5',
      from: '0xmain',
      to: '0xrouter',
      protocol: 'uniswap',
      hash: HASH(5),
      gas: '0.004',
    },
    {
      id: 'cev:6',
      time: '2026-11-06T09:00:00Z',
      kind: 'transfer',
      direction: 'in',
      symbol: 'BTC',
      assetKey: 'fiat|btc',
      amount: '0.01',
      from: '',
      to: '',
      exchange: 'bybit',
      hash: 'ex-77001',
    },
  ]
  // The size the spec asks for: ten thousand operations, deterministic.
  const bulk: MockEvent[] = Array.from({ length: 9992 }, (_, index) => {
    const when = new Date(Date.UTC(2026, 10, 5) - index * 3_600_000 * 2)
    return {
      id: `cev:g${index}`,
      time: when.toISOString().replace('.000Z', 'Z'),
      kind: index % 7 === 0 ? 'swap' : 'transfer',
      direction: index % 2 ? 'in' : 'out',
      symbol: index % 3 ? 'USDT' : 'ETH',
      assetKey: index % 3 ? `ethereum|${USDT}` : 'ethereum|native',
      amount: String(1 + (index % 40)),
      from: index % 2 ? `0x${(index % 997).toString(16).padStart(40, '0')}` : '0xmain',
      to: index % 2 ? '0xmain' : `0x${(index % 991).toString(16).padStart(40, '0')}`,
      protocol: index % 7 === 0 ? 'uniswap' : undefined,
      hash: HASH(100 + index),
    }
  })
  return [...head, ...bulk]
}
