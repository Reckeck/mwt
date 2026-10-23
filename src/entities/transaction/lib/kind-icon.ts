import type { Component } from 'vue'
import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpRight,
  CandlestickChart,
  Layers,
  Scale,
  ShoppingBag,
  TrendingUp,
} from 'lucide-vue-next'

/** The square mark of a row on the phone, where the kind's word does not fit (Ops-1 · 1). */
export const TX_KIND_ICON: Record<TxKind, Component> = {
  payment: ArrowUpRight,
  topup: ArrowDownLeft,
  transfer: ArrowRight,
  convert: ArrowLeftRight,
  trade_spot: TrendingUp,
  trade_futures: CandlestickChart,
  defi: Layers,
  asset_sell: ShoppingBag,
  adjustment: Scale,
}
