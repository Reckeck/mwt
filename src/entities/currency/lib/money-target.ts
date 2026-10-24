/** Every `<Money>` that can show itself in other currencies. */
export const MONEY_SUM = '[data-money-convertible]'

/** A sum inside one of these already does something on a click: it keeps the hover tooltip instead. */
const PRESSABLE =
  'a, button, label, input, select, textarea, [role="button"], [role="menuitem"], [role="option"], [role="tab"]'

/** The sum a node belongs to, if any. */
export function sumOf(node: EventTarget | null): HTMLElement | null {
  return node instanceof Element ? node.closest<HTMLElement>(MONEY_SUM) : null
}

/** A sum a click opens as a card (Currency-1 · 5): not one inside a button or a link. */
export function isPressableSum(sum: HTMLElement): boolean {
  return !sum.parentElement?.closest(PRESSABLE)
}

/** What a sum says about itself through its data attributes. */
export interface SumData {
  amount: number
  currency: string
  /** The operation's moment, RFC 3339; empty for a balance or a total of now. */
  at: string
}

export function sumData(sum: HTMLElement): SumData {
  return {
    amount: Number(sum.dataset.moneyAmount),
    currency: sum.dataset.moneyCurrency ?? '',
    at: sum.dataset.moneyAt ?? '',
  }
}
