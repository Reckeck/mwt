import type { Component } from 'vue'
import {
  ArrowLeftRight,
  Banknote,
  ChartCandlestick,
  Coins,
  CreditCard,
  House,
  Landmark,
  PiggyBank,
  Repeat,
  Sprout,
  TrendingUp,
  Vault,
  Wallet,
} from 'lucide-vue-next'

export function getStoreIcon(kind: StoreKind): Component {
  switch (kind) {
    case 'exchange':
      return ArrowLeftRight
    case 'crypto':
      return Wallet
    case 'bank':
      return Landmark
    case 'cash':
      return Banknote
    case 'property':
      return House
    case 'liability':
      return CreditCard
    default:
      return Wallet
  }
}

export function getSubTypeIcon(subType: SubType): Component {
  switch (subType) {
    case 'card':
    case 'credit_card':
      return CreditCard
    case 'account':
      return Landmark
    case 'deposit':
      return Vault
    case 'savings':
      return PiggyBank
    case 'token':
      return Coins
    case 'spot':
      return ChartCandlestick
    case 'futures':
      return TrendingUp
    case 'earn':
      return Sprout
    case 'pair':
      return Repeat
    default:
      return Banknote
  }
}

/** The icon of any store: its sub-account type when it has one, else its kind. */
export function storeIcon(store: Pick<Store, 'kind' | 'sub_type'>): Component {
  return store.sub_type ? getSubTypeIcon(store.sub_type) : getStoreIcon(store.kind)
}
