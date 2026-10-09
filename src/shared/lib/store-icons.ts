import type { Component } from 'vue'
import { ArrowLeftRight, Banknote, CreditCard, House, Landmark, Lock, Wallet } from 'lucide-vue-next'

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
    case 'asset':
      return House
    case 'liability':
      return CreditCard
    default:
      return Wallet
  }
}
