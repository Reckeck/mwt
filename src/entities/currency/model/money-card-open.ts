import { ref } from 'vue'

import type { SumData } from '../lib/money-target'

/** A request to show the sum card from code (a menu item), not from a click on a sum. */
export interface MoneyCardRequest {
  sum: SumData
  /** The sum on screen the card sits by on a computer; a phone's sheet needs none. */
  anchor: HTMLElement | null
}

/** Read by `MoneyCardHost`, the one place that draws the card. */
export const moneyCardRequest = ref<MoneyCardRequest | null>(null)

/** Show the sum card for `sum`, by `anchor` on a computer (Currency-1 · 5). */
export function showMoneyCard(sum: SumData, anchor: HTMLElement | null = null) {
  moneyCardRequest.value = { sum, anchor }
}
