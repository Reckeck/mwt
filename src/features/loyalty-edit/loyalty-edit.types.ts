import type { LoyaltyField } from './model/use-loyalty-info'

/** What «Новая программа» checks: its own fields on top of the programme's points. */
export type ProgramField = LoyaltyField | 'name' | 'wallet'
