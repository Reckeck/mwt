/**
 * A year of everyday money for the `year` and `stress` scenarios: a salary of about
 * 150 000 ₽ (≈ 1 550 $) each month, fixed bills, weekly groceries and the rest over
 * 13 categories; December spikes on gifts, August goes into the red on a trip.
 * Seeded, so every run (and every screenshot) shows the same numbers.
 */

const RUB = { symbol: 'RUB', name: 'Рубль', coingecko_id: null, chain: 'fiat' as Chain, contract: null }
const DETAILS: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }

/** A spend: category, comment, monthly amount range in ₽, how many times a month. */
type Spend = [category: string, comment: string, low: number, high: number, times: number]

const MONTHLY: Spend[] = [
  ['rent_mortgage', 'Аренда квартиры', 45000, 45000, 1],
  ['utilities', 'ЖКХ', 5200, 7800, 1],
  ['internet', 'Интернет', 900, 900, 1],
  ['mobile', 'Связь', 650, 650, 1],
  ['subscriptions', 'Подписки', 1200, 1900, 1],
  ['groceries', 'Пятёрочка', 2600, 4400, 8],
  ['food_delivery', 'Доставка', 900, 1800, 4],
  ['transport', 'Метро', 800, 1200, 3],
  ['taxi', 'Такси', 450, 1100, 4],
  ['medicine', 'Аптека', 600, 2600, 1],
  ['clothing', 'Одежда', 2500, 7500, 1],
  ['entertainment', 'Кино и кафе', 1500, 3200, 2],
]

/** Months with a spike (0 = January): December gifts, an August trip. */
const SPIKES: Record<number, Spend[]> = {
  11: [
    ['gifts', 'Подарки к Новому году', 28000, 36000, 1],
    ['entertainment', 'Корпоратив и праздники', 6000, 9000, 1],
  ],
  7: [['travel', 'Отпуск на море', 165000, 180000, 1]],
}

/** mulberry32: a tiny seeded generator. */
function seeded(seed: number) {
  let state = seed
  return () => {
    state = (state + 0x6d2b79f5) | 0
    let value = Math.imul(state ^ (state >>> 15), 1 | state)
    value = (value + Math.imul(value ^ (value >>> 7), 61 | value)) ^ value
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296
  }
}

function common(at: Date, comment: string, categories: string[]): TxCommonInput {
  return {
    timestamp: at.toISOString(),
    comment,
    categories,
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
  }
}

/**
 * The inputs for the last twelve months on `bankId`, up to `now`. `scale` multiplies
 * every amount (the stress scenario shows millions). The current month's salary is
 * left to the demo history, which already has one.
 */
export function yearInputs(bankId: string, now: Date, scale = 1): TransactionInput[] {
  const random = seeded(20261023)
  const between = (low: number, high: number) => Math.round((low + (high - low) * random()) / 10) * 10
  const inputs: TransactionInput[] = []
  const payment = (at: Date, [category, comment]: Spend, amount: number) =>
    inputs.push({
      type: 'payment',
      common: common(at, comment, [category]),
      store_id: bankId,
      asset: RUB,
      amount: String(amount * scale),
      fee: null,
      to: { raw: comment },
      details: DETAILS,
    })

  for (let back = 11; back >= 0; back--) {
    const month = new Date(now.getFullYear(), now.getMonth() - back, 1)
    const days = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate()
    const day = (dayOfMonth: number, hour: number) =>
      new Date(month.getFullYear(), month.getMonth(), Math.min(dayOfMonth, days), hour, 0, 0)
    if (back > 0) {
      inputs.push({
        type: 'topup',
        common: common(day(5, 10), 'Зарплата', ['salary']),
        store_id: bankId,
        asset: RUB,
        amount: String(between(145000, 157000) * scale),
        fee: null,
        from: null,
        details: DETAILS,
      })
    }
    for (const spend of [...MONTHLY, ...(SPIKES[month.getMonth()] ?? [])]) {
      const [, , low, high, times] = spend
      for (let index = 0; index < times; index++) {
        const at = day(1 + Math.floor(((index + random()) * days) / times), 9 + Math.floor(random() * 12))
        if (at > now) continue
        payment(at, spend, between(low, high))
      }
    }
  }
  return inputs
}

/** Long names, a currency without a rate: what overflows first. */
export const STRESS_STORE_NAME = 'Накопительный счёт «Копилка на квартиру и непредвиденные расходы»'
export const STRESS_NO_RATE = {
  symbol: 'KZT',
  name: 'Тенге',
  coingecko_id: null,
  chain: 'fiat' as Chain,
  contract: null,
}
export const STRESS_DETAILS = DETAILS
