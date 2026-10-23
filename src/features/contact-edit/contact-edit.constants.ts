import type { Component } from 'vue'
import { CircleDollarSign, CreditCard, Hash, Landmark, Link, Smartphone } from 'lucide-vue-next'

export const CONTACT_KINDS: ContactKind[] = ['person', 'company', 'exchanger']

export const DETAIL_KINDS: DetailKind[] = ['card', 'account', 'iban', 'crypto_address', 'phone', 'url', 'other']

/** The icon of a requisite row: the kind is read before the number. */
export const DETAIL_ICONS: Record<DetailKind, Component> = {
  card: CreditCard,
  account: Landmark,
  iban: Landmark,
  crypto_address: CircleDollarSign,
  phone: Smartphone,
  url: Link,
  other: Hash,
}
