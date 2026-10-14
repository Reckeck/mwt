/** Simple moving average; `null` until there are `period` values. */
export function sma(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  let sum = 0
  for (let index = 0; index < values.length; index++) {
    sum += values[index]
    if (index >= period) sum -= values[index - period]
    out.push(index >= period - 1 ? sum / period : null)
  }
  return out
}

/** Exponential moving average, seeded with the SMA of the first `period` values. */
export function ema(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  const multiplier = 2 / (period + 1)
  let prev: number | null = null
  for (let index = 0; index < values.length; index++) {
    if (index < period - 1) {
      out.push(null)
      continue
    }
    if (prev === null) {
      let seed = 0
      for (let from = index - period + 1; from <= index; from++) seed += values[from]
      prev = seed / period
      out.push(prev)
      continue
    }
    prev = values[index] * multiplier + prev * (1 - multiplier)
    out.push(prev)
  }
  return out
}

/** Wilder's RSI over `period` changes; `null` until there are enough. */
export function rsi(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = new Array(values.length).fill(null)
  if (values.length <= period) return out
  let gain = 0
  let loss = 0
  for (let index = 1; index <= period; index++) {
    const delta = values[index] - values[index - 1]
    if (delta >= 0) gain += delta
    else loss -= delta
  }
  let avgGain = gain / period
  let avgLoss = loss / period
  const toRsi = () => (avgLoss === 0 ? 100 : 100 - 100 / (1 + avgGain / avgLoss))
  out[period] = toRsi()
  for (let index = period + 1; index < values.length; index++) {
    const delta = values[index] - values[index - 1]
    avgGain = (avgGain * (period - 1) + Math.max(delta, 0)) / period
    avgLoss = (avgLoss * (period - 1) + Math.max(-delta, 0)) / period
    out[index] = toRsi()
  }
  return out
}
