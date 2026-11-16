/**
 * What the operations of the crypto module may use from this slice: the amount component. A module
 * operation is money, and money is printed by `<Money>` everywhere in the app (rule 15), so the two
 * entity slices meet here, through this door, and not by reaching into each other (FSD, rule 9).
 */
export { Money } from '../ui/money'
export { PnlText } from '../ui/pnl-text'
