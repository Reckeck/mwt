import type { Component } from 'vue'
import {
  ArrowLeftRight,
  ArrowRightLeft,
  Banknote,
  Car,
  ChartCandlestick,
  Coins,
  CreditCard,
  Gift,
  House,
  Landmark,
  Package,
  PiggyBank,
  Repeat,
  Sprout,
  Trees,
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
    case 'loyalty':
      return Gift
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
    case 'pds':
      return Landmark
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
    case 'funding':
      return ArrowRightLeft
    case 'unified':
      return ChartCandlestick
    default:
      return Banknote
  }
}

export function getPropertyIcon(category: PropertyCategory): Component {
  switch (category) {
    case 'car':
      return Car
    case 'land':
      return Trees
    case 'other':
      return Package
    default:
      return House
  }
}

/** The icon of any store: its sub-account type, else its property type, else its kind. */
export function storeIcon(store: Pick<Store, 'kind' | 'sub_type'> & { property?: PropertyInfo | null }): Component {
  if (store.sub_type) return getSubTypeIcon(store.sub_type)
  if (store.property) return getPropertyIcon(store.property.category)
  return getStoreIcon(store.kind)
}
