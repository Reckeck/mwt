import type { Component } from 'vue'
import { ArrowLeftRight, Banknote, Landmark, Lock, Wallet } from 'lucide-vue-next'

export type StoreKind = 'exchange' | 'hot_wallet' | 'cold_wallet' | 'bank' | 'cash'

export function getStoreIcon(kind: StoreKind): Component {
  switch (kind) {
    case 'exchange':
      return ArrowLeftRight
    case 'hot_wallet':
      return Wallet
    case 'cold_wallet':
      return Lock
    case 'bank':
      return Landmark
    case 'cash':
      return Banknote
    default:
      return Wallet
  }
}
