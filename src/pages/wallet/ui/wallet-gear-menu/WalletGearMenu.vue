<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import {
  ArrowLeftRight,
  Calculator,
  FolderPlus,
  Goal as GoalIcon,
  Plus,
  RefreshCw,
  Repeat,
  Scale,
  Settings2,
  SlidersHorizontal,
} from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { openConverter } from '@/shared/lib/converter'

/**
 * The gear at the end of the header (Wallets-1 · 5): the wallet's settings
 * and the rarer actions — reconcile, prices, calculator, a sub-account, a goal
 * or an automation from this wallet.
 */
const props = defineProps<{
  store: Store
  canAddChild: boolean
  hasChildren: boolean
  refreshing: boolean
  /** A property has no kind buttons yet: «+ Операция» waits here. */
  operation: boolean
}>()
const emit = defineEmits<{
  operation: []
  adjust: []
  calculator: []
  subAccount: []
  goal: []
  automation: []
  refresh: []
}>()

const { t } = useI18n()
const router = useRouter()

/** A wallet bound to one fiat currency starts the converter from it. */
const ownCurrency = computed(() => {
  const binding = props.store.binding
  return binding && typeof binding === 'object' && 'asset' in binding && binding.asset.chain === 'fiat'
    ? binding.asset.symbol.toLowerCase()
    : null
})
const moneyWallet = computed(() => ['bank', 'cash', 'liability'].includes(props.store.kind))
const live = computed(() => !props.store.archived)
</script>

<template>
  <DropdownMenu align="end">
    <template #trigger>
      <Button variant="outline" size="icon" :aria-label="t('wallet.gear')" :title="t('wallet.gear')" data-tour="wallet-gear">
        <SlidersHorizontal class="size-4" />
      </Button>
    </template>
    <DropdownMenuItem value="settings" @select="router.push({ name: 'wallet-settings', params: { id: props.store.id } })">
      <Settings2 class="size-4 shrink-0" />
      {{ t('walletSettings.nav') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="props.operation && live" value="operation" @select="emit('operation')">
      <Plus class="size-4 shrink-0" />
      {{ t('wallet.addOperation') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="props.store.kind !== 'property' && live" value="adjust" @select="emit('adjust')">
      <Scale class="size-4 shrink-0" />
      {{ t('adjustment.open') }}
    </DropdownMenuItem>
    <DropdownMenuItem value="refresh" :disabled="props.refreshing" @select="emit('refresh')">
      <RefreshCw class="size-4 shrink-0" />
      {{ t('store.refresh') }}
    </DropdownMenuItem>
    <DropdownMenuItem value="calculator" @select="emit('calculator')">
      <Calculator class="size-4 shrink-0" />
      {{ t('calc.title') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="moneyWallet" value="converter" @select="openConverter(ownCurrency)">
      <ArrowLeftRight class="size-4 shrink-0" />
      {{ t('palette.converter') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="props.canAddChild && !props.hasChildren" value="sub-account" @select="emit('subAccount')">
      <FolderPlus class="size-4 shrink-0" />
      {{ t('stores.newSubAccountShort') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="live" value="goal" @select="emit('goal')">
      <GoalIcon class="size-4 shrink-0" />
      {{ t('goal.makeFromStore') }}
    </DropdownMenuItem>
    <DropdownMenuItem v-if="live" value="automation" @select="emit('automation')">
      <Repeat class="size-4 shrink-0" />
      {{ t('automation.makeFromStore') }}
    </DropdownMenuItem>
  </DropdownMenu>
</template>
