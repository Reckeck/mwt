<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatDayMonthShort, formatMoney } from '@/shared/lib/format'
import { getStoreIcon, storeIcon } from '@/shared/lib/store-icons'
import { txLegForStore, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { Separator } from '@/shared/ui/separator'

import { useWalletGlance } from '../model/use-wallet-glance'
import type { WalletGlanceBodyProps } from '../quick-glance.types'

/** A wallet at a glance (Paths 1 · 4): its value, sub-accounts and last three operations. */
const props = defineProps<WalletGlanceBodyProps>()
const { t } = useI18n()

const glance = useWalletGlance(() => props.storeId, () => true)

const money = (value: string) => formatMoney(value, { currency: glance.currency.value })
const subtitle = computed(() => {
  const kind = glance.store.value ? t(`stores.kind.${glance.store.value.kind}`) : ''
  return [kind, glance.assets.value.join(', ')].filter(Boolean).join(' · ')
})
const recent = computed(() =>
  glance.recent.value.map((tx) => {
    const meta = txMeta(tx)
    const legs = txPrincipalLegs(tx)
    const leg = txLegForStore(tx, props.storeId) ?? legs.at(-1)
    const amount = leg ? formatMoney(leg.amount, { currency: leg.asset.symbol, signed: Number(leg.amount) > 0 }) : ''
    const income = Number(leg?.amount ?? 0) > 0 && legs.length > 1
    return { id: meta.id, income, name: meta.comment || legs.map((one) => one.asset.symbol).join(' → '), day: formatDayMonthShort(meta.timestamp), amount }
  }),
)

const icon = computed(() => (glance.store.value ? storeIcon(glance.store.value) : getStoreIcon('bank')))
</script>

<template>
  <div class="flex items-start gap-2.5">
    <span class="grid size-8 shrink-0 place-items-center rounded-md bg-muted text-muted-foreground">
      <component :is="icon" class="size-4" aria-hidden="true" />
    </span>
    <div class="flex min-w-0 flex-1 flex-col">
      <b class="truncate">{{ props.name }}</b>
      <span v-if="subtitle" class="truncate text-xs text-muted-foreground">{{ subtitle }}</span>
    </div>
    <b class="shrink-0 text-lg tabular-nums">{{ money(glance.total.value) }}</b>
  </div>
  <div v-if="glance.children.value.length" class="grid grid-cols-2 gap-2">
    <div v-for="child in glance.children.value" :key="child.store.id" class="flex min-w-0 flex-col rounded-lg border px-2.5 py-2">
      <span class="truncate text-xs text-muted-foreground">{{ child.store.name }}</span>
      <b class="tabular-nums">{{ money(child.subtree_base) }}</b>
    </div>
  </div>
  <template v-if="recent.length">
    <Separator />
    <span class="text-xs text-muted-foreground">{{ t('glance.wallet.last') }}</span>
    <div v-for="row in recent" :key="row.id" class="flex items-baseline gap-2">
      <span class="min-w-0 flex-1 truncate">{{ row.name }} <span class="text-muted-foreground">· {{ row.day }}</span></span>
      <b class="tabular-nums" :class="row.income && 'text-success'">{{ row.amount }}</b>
    </div>
  </template>
</template>
