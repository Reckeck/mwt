<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ChevronLeft } from 'lucide-vue-next'

import { CryptoAddressCard } from '@/entities/crypto-account'
import { Money, PnlText } from '@/entities/currency'
import { walletSubtitle } from '@/entities/store'
import { type PageCrumb, PageHeader } from '@/shared/ui/page-header'
import { Button } from '@/shared/ui/button'
import { formatMoney } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'
import { storeIcon } from '@/shared/lib/store-icons'

import { walletActions } from '../../lib/wallet-actions'
import type { WalletFigure } from '../../lib/wallet-figure'
import { WalletActions } from '../wallet-actions'
import { WalletGearMenu } from '../wallet-gear-menu'

/**
 * The top of a wallet (Wallets-1 · 5–7, Currency-1 · 3–4): its sum as its bank
 * shows it, the base and the rate beside, the buttons its kind allows and the
 * gear with the rest. Its name is the top bar's title.
 */
const props = defineProps<{
  store: Store
  ancestors: StoreCrumb[]
  figure: WalletFigure
  canAddChild: boolean
  /** The sub-accounts card offers a new one already. */
  hasChildren: boolean
  refreshing: boolean
  /** An exchange's own wallet: its buttons open `Trade`. */
  exchange?: boolean
  /** Nothing in it yet (Wallets-3 · 18): the first step below replaces the buttons; a phone keeps only the title. */
  empty?: boolean
  /** The wallet is driven by the crypto engine: it only reads, so no button here creates an operation (spec E9). */
  engine?: boolean
  /** The followed account of an address wallet, for the card; `null` while it loads. */
  account?: CryptoAccountView | null
  /** When the account was last read (RFC3339). */
  readAt?: string | null
}>()
const emit = defineEmits<{
  act: [kind: TxKind]
  settings: []
  operation: []
  adjust: []
  calculator: []
  subAccount: []
  goal: []
  automation: []
  refresh: []
  revalue: []
  sell: []
}>()

const { t } = useI18n()
const phone = useIsPhone()
const router = useRouter()

/** The way up: `Wallets ›`, then the wallets above this one; a top-level wallet ends with its own name. */
const crumbs = computed<PageCrumb[]>(() => [
  { label: t('nav.stores'), to: { name: 'stores' } },
  ...props.ancestors.map((parent) => ({ label: parent.name, to: { name: 'wallet', params: { id: parent.id } } })),
  ...(props.ancestors.length ? [] : [{ label: props.store.name }]),
])
/** A root wallet on a phone: «‹» to the list instead of a trail. */
const rootOnPhone = computed(() => phone.value && !props.ancestors.length)

const actions = computed(() => (props.engine ? [] : walletActions(props.store, Boolean(props.exchange))))
/** An address wallet shows the address card instead of the icon and the plain sum (mockups 01 and 02). */
const addressCard = computed(() => props.engine && props.store.kind === 'crypto' && props.account)
/** Property (Wallets-2 · 10): `value not set · Estimate` until it has one, and `Sell`. */
const property = computed(() => props.store.kind === 'property' && !props.store.parent_id)
/** `rate 93.75 ₽ · Bank · in the budget` or `Exchange · BTC, USDT`; a phone drops the rate, property its asset's code. */
// money: plain — an exchange rate, a price of one unit rather than a sum to convert
const subtitle = computed(() => {
  const own = props.figure.base
  const rate = own?.rate && !phone.value ? [t('wallet.rate', { rate: formatMoney(own.rate, { currency: props.figure.currency }) })] : []
  const assets = own || property.value || !props.figure.assets.length ? [] : [props.figure.assets.join(', ')]
  const driven = props.engine && !addressCard.value ? [t('cryptoTrack.accounts.engineDriven')] : []
  return [...rate, walletSubtitle(props.store, t), ...driven, ...assets].join(' · ')
})
const unvalued = computed(() => property.value && !Number(props.figure.amount))
</script>

<template>
  <div class="flex flex-col gap-3">
    <Button
      v-if="rootOnPhone"
      variant="ghost"
      size="icon"
      class="touch-hit -ms-2 self-start"
      :aria-label="t('nav.stores')"
      data-wallet-back
      @click="router.push({ name: 'stores' })"
    >
      <ChevronLeft class="size-5" />
    </Button>
    <PageHeader v-else :crumbs="crumbs" />

    <div v-if="!(props.empty && phone)" class="flex items-center gap-3">
      <CryptoAddressCard
        v-if="addressCard"
        class="min-w-0 flex-1"
        :name="addressCard.name"
        :chain="addressCard.chain"
        :address="addressCard.address"
        :source="addressCard.source"
        :read-at="props.readAt ?? null"
      >
        <span class="text-2xl font-semibold" data-wallet-sum>
          <Money glance :amount="props.figure.amount" :currency="props.figure.currency" :decimals="2" />
        </span>
        <span v-if="props.figure.base" class="text-caption text-muted-foreground">
          ≈ <Money glance :amount="props.figure.base.amount" :decimals="2" />
        </span>
        <!-- Only from two real snapshots a month apart; with no such pair the line is not drawn at all. -->
        <span v-if="addressCard.month_change" class="text-caption text-muted-foreground" data-crypto-month-change>
          <PnlText :amount="addressCard.month_change.change_usd" currency="usd" :pct="addressCard.month_change.change_pct ?? null" :decimals="2" />
          · {{ t('cryptoTrack.accounts.perMonth') }}
        </span>
      </CryptoAddressCard>
      <span v-else class="flex size-12 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground max-md:hidden">
        <component :is="storeIcon(props.store)" class="size-5" />
      </span>
      <div v-if="!addressCard" class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="text-xl font-semibold tabular-nums max-md:text-[1.75rem] max-md:leading-tight" data-wallet-sum>
          <Money glance :amount="props.figure.amount" :currency="props.figure.currency" :decimals="2" />
        </span>
        <span class="truncate text-caption text-muted-foreground">
          <template v-if="props.figure.base">≈ <Money glance :amount="props.figure.base.amount" :decimals="2" /> · </template>
          {{ subtitle }}
          <template v-if="unvalued">
            · {{ t('wallet.valueUnset') }} ·
            <Button variant="link" class="h-auto p-0 text-caption touch-hit-xl" :disabled="props.store.archived" @click="emit('revalue')">
              {{ t('wallet.revalue') }}
            </Button>
          </template>
        </span>
      </div>
      <WalletActions
        v-if="!phone && !props.empty && !props.engine"
        :actions="actions"
        :phone="false"
        :disabled="props.store.archived"
        @act="(kind) => emit('act', kind)"
      />
      <Button v-if="property && !phone" variant="outline" :disabled="props.store.archived" data-wallet-sell @click="emit('sell')">{{ t('wallet.sell') }}</Button>
      <WalletGearMenu
        :store="props.store"
        :can-add-child="props.canAddChild"
        :has-children="props.hasChildren"
        :refreshing="props.refreshing"
        :operation="!actions.length && !props.engine"
        :engine="props.engine"
        :sell="property && phone"
        @settings="emit('settings')"
        @operation="emit('operation')"
        @sell="emit('sell')"
        @adjust="emit('adjust')"
        @calculator="emit('calculator')"
        @sub-account="emit('subAccount')"
        @goal="emit('goal')"
        @automation="emit('automation')"
        @refresh="emit('refresh')"
        @positions="router.push({ name: 'crypto-positions' })"
      />
    </div>

    <WalletActions v-if="phone && !props.empty && !props.engine" :actions="actions" phone :disabled="props.store.archived" @act="(kind) => emit('act', kind)" />

    <p v-if="props.store.archived" class="rounded-md border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
      {{ t('wallet.archivedHint') }}
    </p>
  </div>
</template>
