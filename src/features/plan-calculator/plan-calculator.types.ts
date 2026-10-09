/** A calculated plan the calculator hands to whatever records it. */
export interface CalculatorPlan {
  mode: CalcMode
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  /** USD price of `from`. */
  price: string
}

export interface PlanCalculatorProps {
  baseCurrency: string
}
