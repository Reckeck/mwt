/** Every `<Money>` that can show itself in other currencies. */
export const MONEY_SUM = '[data-money-convertible]'

/** A sum inside one of these already does something on a click: a click goes to the button, a hold opens the card. */
const PRESSABLE =
  'a, button, label, input, select, textarea, [role="button"], [role="menuitem"], [role="option"], [role="tab"]'

/** The sum a node belongs to, if any. */
export function sumOf(node: EventTarget | null): HTMLElement | null {
  return node instanceof Element ? node.closest<HTMLElement>(MONEY_SUM) : null
}

/**
 * The sum a keyboard focus is on: the node's own, or the one inside the focused button or link.
 * A panel that holds sums (a dialog taking focus as it opens) is not a sum's button.
 */
export function sumOfFocus(node: Element): HTMLElement | null {
  return sumOf(node) ?? (node.matches(PRESSABLE) ? node.querySelector<HTMLElement>(MONEY_SUM) : null)
}

/**
 * The sum under a finger at `point`: the node itself, or — a button's enlarged touch area is the
 * event's target, not the sum drawn on it — a sum inside the pressed button that holds the point.
 */
export function sumAtPoint(node: EventTarget | null, point: { x: number; y: number }): HTMLElement | null {
  const own = sumOf(node)
  if (own || !(node instanceof Element)) return own
  const holder = node.closest(PRESSABLE)
  const inside = (rect: DOMRect) =>
    point.x >= rect.left - 6 && point.x <= rect.right + 6 && point.y >= rect.top - 6 && point.y <= rect.bottom + 6
  return (
    [...(holder?.querySelectorAll<HTMLElement>(MONEY_SUM) ?? [])].find((sum) => inside(sum.getBoundingClientRect())) ??
    null
  )
}

/** A sum a click opens as a card (Currency-1 · 5): true for one not inside a button or a link. */
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
