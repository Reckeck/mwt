/** «Подушка безопасности» of the demo vault (New-Goal 3): saved in cash, filled by «10 % зарплаты», with a file and a link. */

import { mockAttachmentData, mockAttachments } from '../attachments'
import { mockEnvelopes } from '../catalog'
import { mockStores, mockTransactions, priceOf } from '../demo'
import { mockTransactionFrom } from '../ledger'
import { seedShare } from './share-seed'
import { goals, iso, now, uuid } from './state'

const USD: AssetRef = {
  symbol: 'USD',
  name: 'US Dollar',
  coingecko_id: null,
  chain: null,
  contract: null,
  quote: 'usd',
}

const monthsFrom = (months: number) => {
  const date = now()
  date.setDate(1)
  date.setMonth(date.getMonth() + months)
  return iso(date).slice(0, 10)
}

/** «10 % зарплаты» put aside three times since the start: the card's «История». */
function seedDeposits(from: string, to: string): Transaction[] {
  const booked: Transaction[] = []
  for (const months of [-2, -1, 0]) {
    const at = new Date(`${monthsFrom(months)}T09:00:00Z`)
    at.setDate(Math.min(25, now().getDate()))
    const input: TransactionInput = {
      type: 'transfer',
      common: {
        timestamp: at.toISOString(),
        comment: '10 % зарплаты → Подушка',
        categories: [],
        flow_id: null,
        contact_id: null,
        price_usd: null,
        fx_override: null,
        price: null,
        price_currency: null,
      },
      from: { store: from },
      to: { store: to },
      asset: USD,
      amount: '157.5',
      fee: null,
      network: null,
      tx_hash: null,
      debt: null,
    }
    booked.push(mockTransactionFrom(input, priceOf))
  }
  mockTransactions.push(...booked)
  return booked
}

export function seedGoal() {
  const cash = mockStores.find((store) => store.kind === 'cash')
  const bank = mockStores.find((store) => store.kind === 'bank')
  if (!cash || !bank) return
  const deposits = seedDeposits(bank.id, cash.id)
  const created = iso(now())
  const goal: Goal = {
    id: uuid(),
    name: 'Подушка безопасности',
    active: true,
    target_amount: '5000',
    currency: 'USD',
    starts_at: `${monthsFrom(-3)}T00:00:00Z`,
    deadline: `${monthsFrom(9)}T00:00:00Z`,
    priority: 4,
    monthly_commitment: null,
    scope: { store_ids: [cash.id], include_children: true },
    created_at: created,
    scenario_id: null,
    expected_return: null,
  }
  goals.push(goal)
  mockEnvelopes.push({
    id: uuid(),
    store_id: cash.id,
    name: goal.name,
    asset: USD,
    allocated: '1539',
    target: goal.target_amount,
    goal_id: goal.id,
    created_at: created,
  })
  seedShare(bank.id, cash.id, deposits)
  const file = uuid()
  mockAttachmentData.set(file, btoa('%PDF-1.7 mock fund'))
  const owner: EntityRef = { kind: 'goal', id: goal.id }
  const base = { owner, sha256: null, created_at: created, removed_at: null }
  mockAttachments.push(
    { ...base, id: file, kind: 'file', title: 'Расчёт подушки.pdf', url: null, mime: 'application/pdf', size: 56_000 },
    {
      ...base,
      id: uuid(),
      kind: 'link',
      title: 'Статья: сколько держать в подушке',
      url: 'https://www.tinkoff.ru/journal/emergency-fund',
      mime: null,
      size: 0,
    },
  )
}
