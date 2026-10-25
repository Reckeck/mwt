import { type Component, type InjectionKey, inject, provide } from 'vue'

/**
 * What a wallet's name and «авто» in an operation row open on the spot. The
 * glances live in a widget, the list in a lower layer: the shell hands them
 * down once, and every list of operations picks them up.
 */
export interface TxGlances {
  wallet: Component
  automation: Component
}

const TX_GLANCES: InjectionKey<TxGlances> = Symbol('tx-glances')

export function provideTxGlances(glances: TxGlances): void {
  provide(TX_GLANCES, glances)
}

export function useTxGlances(): TxGlances | null {
  return inject(TX_GLANCES, null)
}
