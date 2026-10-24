<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
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
  /** An exchange's own wallet: its buttons open «Сделка». */
  exchange?: boolean
  /** Nothing in it yet (Wallets-3 · 18): the first step below replaces the buttons; a phone keeps only the title. */
  empty?: boolean
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

/** A sub-account shows the way up to it; a wallet of the top level needs none. */
const crumbs = computed<PageCrumb[]>(() => [
  { label: t('nav.stores'), to: { name: 'stores' } },
  ...props.ancestors.map((parent) => ({ label: parent.name, to: { name: 'wallet', params: { id: parent.id } } })),
])

const actions = computed(() => walletActions(props.store, Boolean(props.exchange)))
/** Property (Wallets-2 · 10): «стоимость не задана · Оценить» until it has one, and «Продать». */
const property = computed(() => props.store.kind === 'property' && !props.store.parent_id)
/** «курс 93,75 ₽ · Банк · в бюджете» or «Биржа · BTC, USDT»; a phone drops the rate, property its asset's code. */
const subtitle = computed(() => {
  const own = props.figure.base
  const rate = own?.rate && !phone.value ? [t('wallet.rate', { rate: formatMoney(own.rate, { currency: props.figure.currency }) })] : []
  const assets = own || property.value || !props.figure.assets.length ? [] : [props.figure.assets.join(', ')]
  return [...rate, walletSubtitle(props.store, t), ...assets].join(' · ')
})
const unvalued = computed(() => property.value && !Number(props.figure.amount))
</script>

<template>
  <div class="flex flex-col gap-3">
    <PageHeader v-if="props.ancestors.length" :crumbs="crumbs" />

    <div v-if="!(props.empty && phone)" class="flex items-center gap-3">
      <span class="flex size-12 shrink-0 items-center justify-center rounded-[10px] bg-muted text-muted-foreground max-md:hidden">
        <component :is="storeIcon(props.store)" class="size-5" />
      </span>
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class="text-xl font-semibold tabular-nums max-md:text-[1.75rem] max-md:leading-tight" data-wallet-sum>
          <Money :amount="props.figure.amount" :currency="props.figure.currency" :decimals="2" />
        </span>
        <span class="truncate text-caption text-muted-foreground">
          <template v-if="props.figure.base">≈ <Money :amount="props.figure.base.amount" :decimals="2" /> · </template>
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
        v-if="!phone && !props.empty"
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
        :operation="!actions.length"
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
      />
    </div>

    <WalletActions v-if="phone && !props.empty" :actions="actions" phone :disabled="props.store.archived" @act="(kind) => emit('act', kind)" />

    <p v-if="props.store.archived" class="rounded-md border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
      {{ t('wallet.archivedHint') }}
    </p>
  </div>
</template>
