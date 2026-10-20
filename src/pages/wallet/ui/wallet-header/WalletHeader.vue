<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowLeftRight, Calculator, FolderPlus, Goal as GoalIcon, Plus, RefreshCw, Repeat, Scale, Settings2 } from 'lucide-vue-next'

import { bindingBadge } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button, buttonVariants } from '@/shared/ui/button'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { type PageCrumb, PageHeader } from '@/shared/ui/page-header'
import { openConverter } from '@/shared/lib/converter'
import { storeIcon } from '@/shared/lib/store-icons'

/**
 * Where the wallet sits, what it is, and what one does with it: the main
 * action first, matching reality, prices and settings at hand, the rarer
 * ones under «Ещё».
 */
const props = defineProps<{
  store: Store
  ancestors: StoreCrumb[]
  canAddChild: boolean
  /** The sub-accounts card offers a new one already. */
  hasChildren: boolean
  refreshing: boolean
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

function openSettings() {
  void router.push({ name: 'wallet-settings', params: { id: props.store.id } })
}

/** The way up to this wallet; its own name is the heading below and the top bar. */
const crumbs = computed<PageCrumb[]>(() => [
  { label: t('nav.stores'), to: { name: 'stores' } },
  ...props.ancestors.map((parent) => ({ label: parent.name, to: { name: 'wallet', params: { id: parent.id } } })),
])

const moneyWallet = computed(() => ['bank', 'cash', 'liability'].includes(props.store.kind))
/** A wallet bound to one currency starts the converter from it. */
const ownCurrency = computed(() => {
  const binding = props.store.binding
  return binding && typeof binding === 'object' && 'asset' in binding && binding.asset.chain === 'fiat'
    ? binding.asset.symbol.toLowerCase()
    : null
})
</script>

<template>
  <div class="flex flex-col gap-3">
    <PageHeader :crumbs="crumbs">
      <span
        class="flex size-10 shrink-0 items-center justify-center rounded-lg border"
        :style="{ backgroundColor: `${props.store.color}1A`, color: props.store.color, borderColor: `${props.store.color}33` }"
      >
        <component :is="storeIcon(props.store)" class="size-5" />
      </span>
      <div class="flex min-w-0 flex-col">
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="heading-page truncate">{{ props.store.name }}</h2>
          <Badge variant="secondary">
            {{ props.store.sub_type ? t(`subType.${props.store.sub_type}`) : t(`stores.kind.${props.store.kind}`) }}
          </Badge>
          <Badge v-if="bindingBadge(props.store)" variant="outline">{{ bindingBadge(props.store) }}</Badge>
          <Badge v-if="props.store.archived" variant="outline">{{ t('stores.archived') }}</Badge>
          <Badge
            v-if="props.store.purpose !== 'everyday' || props.store.kind !== 'liability'"
            variant="outline"
            :title="t(`purpose.${props.store.purpose}Hint`)"
          >
            {{ t(`purpose.${props.store.purpose}`) }}
          </Badge>
        </div>
        <p v-if="props.store.note" class="truncate text-sm text-muted-foreground">{{ props.store.note }}</p>
      </div>

      <template #actions>
        <Button
          v-if="props.store.kind !== 'property'"
          variant="outline"
          size="sm"
          class="max-md:hidden"
          :disabled="props.store.archived"
          data-tour="wallet-adjust"
          @click="emit('adjust')"
        >
          <Scale class="size-4" />
          {{ t('adjustment.open') }}
        </Button>
        <Button
          variant="outline"
          size="icon-sm"
          class="max-md:hidden"
          :title="t('store.refresh')"
          :aria-label="t('store.refresh')"
          :disabled="props.refreshing"
          @click="emit('refresh')"
          :loading="props.refreshing"
        >
          <RefreshCw class="size-4" />
        </Button>
        <RouterLink
          :to="{ name: 'wallet-settings', params: { id: props.store.id } }"
          :class="buttonVariants({ variant: 'outline', size: 'icon' })"
          class="size-8 max-md:hidden"
          :title="t('walletSettings.nav')"
          :aria-label="t('walletSettings.nav')"
          data-tour="wallet-settings"
        >
          <Settings2 class="size-4" />
        </RouterLink>
        <Button size="sm" :disabled="props.store.archived" data-tour="wallet-operation" @click="emit('operation')">
          <Plus class="size-4" />
          {{ t('wallet.addOperation') }}
        </Button>
      </template>

      <template #more>
        <!-- On a phone the header keeps the main action; these move here. -->
        <DropdownMenuItem v-if="props.store.kind !== 'property'" value="adjust" class="md:hidden" :disabled="props.store.archived" @select="emit('adjust')">
          <Scale class="size-4 shrink-0" />
          {{ t('adjustment.open') }}
        </DropdownMenuItem>
        <DropdownMenuItem value="refresh" class="md:hidden" :disabled="props.refreshing" @select="emit('refresh')">
          <RefreshCw class="size-4 shrink-0" />
          {{ t('store.refresh') }}
        </DropdownMenuItem>
        <DropdownMenuItem value="settings" class="md:hidden" @select="openSettings">
          <Settings2 class="size-4 shrink-0" />
          {{ t('walletSettings.nav') }}
        </DropdownMenuItem>
        <DropdownMenuItem value="calculator" @select="emit('calculator')">
          <Calculator class="size-4 shrink-0" />
          {{ t('calc.title') }}
        </DropdownMenuItem>
        <!-- Money wallets convert their own currency; a coin wallet has the terminal. -->
        <DropdownMenuItem v-if="moneyWallet" value="converter" @select="openConverter(ownCurrency)">
          <ArrowLeftRight class="size-4 shrink-0" />
          {{ t('palette.converter') }}
        </DropdownMenuItem>
        <DropdownMenuItem v-if="props.canAddChild && !props.hasChildren" value="sub-account" @select="emit('subAccount')">
          <FolderPlus class="size-4 shrink-0" />
          {{ t('stores.newSubAccountShort') }}
        </DropdownMenuItem>
        <DropdownMenuItem v-if="!props.store.archived" value="goal" @select="emit('goal')">
          <GoalIcon class="size-4 shrink-0" />
          {{ t('goal.makeFromStore') }}
        </DropdownMenuItem>
        <DropdownMenuItem v-if="!props.store.archived" value="automation" @select="emit('automation')">
          <Repeat class="size-4 shrink-0" />
          {{ t('automation.makeFromStore') }}
        </DropdownMenuItem>
      </template>
    </PageHeader>

    <p v-if="props.store.archived" class="rounded-md border border-border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
      {{ t('wallet.archivedHint') }}
    </p>
  </div>
</template>
