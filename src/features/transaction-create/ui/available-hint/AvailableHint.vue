<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { formatQuantity } from '@/shared/lib/format'

/**
 * `Available: 1,250 USDT · All` under an amount that leaves a wallet. Matched by
 * the asset's key; envelopes are not subtracted, the core warns about those.
 */
const props = defineProps<{ storeId: string | null; asset: AssetRef | null }>()
const emit = defineEmits<{ max: [quantity: string] }>()
const { t } = useI18n()

const overview = useQuery({
  // no placeholder: another account's balance
  queryKey: computed(() => ['store', props.storeId, false]),
  queryFn: () => call<StoreOverview>('get_store_overview', { storeId: props.storeId, includeChildren: false }),
  enabled: computed(() => Boolean(props.storeId)),
})
const held = computed(() => {
  if (!props.asset || !overview.data.value) return null
  const key = assetKey(props.asset)
  return overview.data.value.positions.find((position) => assetKey(position.asset) === key)?.quantity ?? '0'
})
</script>

<template>
  <p v-if="held !== null && props.asset" class="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
    <template v-if="Number(held) > 0">
      {{ t('tx.available', { amount: `${formatQuantity(held)} ${props.asset.symbol.toUpperCase()}` }) }}
      <Button type="button" variant="link" size="sm" class="h-auto p-0 text-xs" @click="emit('max', held)">
        {{ t('tx.max') }}
      </Button>
    </template>
    <template v-else>{{ t('tx.availableNone', { symbol: props.asset.symbol.toUpperCase() }) }}</template>
  </p>
</template>
