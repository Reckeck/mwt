<script setup lang="ts">
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { PendingImports } from '@/features/pending-imports'
import { ExchangeBalances } from '@/widgets/exchange-balances'
import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { EmptyState } from '@/shared/ui/empty-state'

/** Trades fetched from the exchange, waiting to be confirmed — or how to connect it first. */
const props = defineProps<{ storeId: string; hasKey: boolean }>()

const { t } = useI18n()
</script>

<template>
  <template v-if="props.hasKey">
    <Card><PendingImports :store-id="props.storeId" /></Card>
    <ExchangeBalances :store-id="props.storeId" />
  </template>
  <EmptyState v-else :title="t('wallet.importNeedsKey')">
    <RouterLink
      :to="{ name: 'wallet-settings', params: { id: props.storeId }, query: { tab: 'exchange' } }"
      :class="buttonVariants({ variant: 'outline', size: 'sm' })"
    >
      {{ t('wallet.openExchangeSettings') }}
    </RouterLink>
  </EmptyState>
</template>
