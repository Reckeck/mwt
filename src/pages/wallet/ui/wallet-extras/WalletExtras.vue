<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ExchangeBalances } from '@/widgets/exchange-balances'
import { LossSummary } from '@/widgets/loss-summary'

/**
 * Coin wallets and exchanges get what is already built for them elsewhere:
 * the losses on their conversions and, on an exchange with a key, the check
 * against its balances (loaded only when asked).
 */
const props = defineProps<{ store: Store; hasKey: boolean }>()
const { t } = useI18n()

const trades = computed(() => props.store.kind === 'crypto' || props.store.kind === 'exchange')
const storeIds = computed(() => [props.store.id])
</script>

<template>
  <template v-if="trades">
    <LossSummary :store-ids="storeIds" :title="t('wallet.losses')" hide-empty />
    <ExchangeBalances v-if="props.store.kind === 'exchange' && !props.store.parent_id && props.hasKey" :store-id="props.store.id" />
  </template>
</template>
