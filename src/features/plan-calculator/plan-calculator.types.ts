/** A calculated plan the calculator hands to whatever records it. */
export interface CalculatorPlan {
  mode: CalcMode
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  /** `position`: units of `to` the amount buys. */
  quantity?: string
  /** `position`: the entry price, in USDT. */
  entryPrice?: string
  /** USD price of `from`; empty when the form should price it itself. */
  price: string
}

export interface PlanCalculatorProps {
  baseCurrency: string
}

export interface PlanCalculatorDialogProps extends PlanCalculatorProps {
  /** The exchange the deal is for: `Work out a trade · Bybit`. */
  exchange?: string | null
}
