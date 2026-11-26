<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { CHAIN_LABELS } from '@/shared/lib/chains'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * The filter of the operations: the address is the search, the network and the kind narrow it, hidden
 * ones come back with a switch. It applies at once; the value lives in the address of the page.
 */
const props = defineProps<{ modelValue: CryptoOpFilter; facets: CryptoOpFacets }>()
const emit = defineEmits<{ 'update:modelValue': [CryptoOpFilter] }>()
const { t } = useI18n()

const ALL = 'all'
const KINDS: CryptoTxKind[] = ['transfer', 'swap', 'approve', 'stake', 'unstake', 'claim', 'bridge', 'contract', 'unknown']
const patch = (change: Partial<CryptoOpFilter>) => emit('update:modelValue', { ...props.modelValue, ...change })
const address = computed({
  get: () => props.modelValue.address ?? '',
  set: (value: string) => patch({ address: value.trim() || undefined }),
})
const chain = computed({
  get: () => props.modelValue.chain ?? ALL,
  set: (value: string) => patch({ chain: value === ALL ? undefined : (value as Chain) }),
})
const kind = computed({
  get: () => props.modelValue.kind ?? ALL,
  set: (value: string) => patch({ kind: value === ALL ? undefined : (value as CryptoTxKind) }),
})
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" data-crypto-op-filter>
    <div class="relative min-w-0 flex-1 basis-56">
      <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
      <Input
        v-model="address"
        class="pl-9"
        :aria-label="t('cryptoTrack.operations.filter.address')"
        :placeholder="t('cryptoTrack.operations.filter.address')"
        data-crypto-filter-address
      />
    </div>
    <Select v-model="chain" class="w-40" :aria-label="t('cryptoTrack.operations.filter.network')">
      <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.network') }}</SelectItem>
      <SelectItem v-for="item in props.facets.chains" :key="item" :value="item">{{ CHAIN_LABELS[item] }}</SelectItem>
    </Select>
    <Select v-model="kind" class="w-40" :aria-label="t('cryptoTrack.operations.filter.kind')">
      <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.kind') }}</SelectItem>
      <SelectItem v-for="item in KINDS" :key="item" :value="item">{{ t(`cryptoTrack.operations.kind.${item}`) }}</SelectItem>
    </Select>
    <SwitchRow
      id="crypto-filter-hidden"
      plain
      class="w-auto"
      :label="t('cryptoTrack.operations.filter.hidden')"
      :model-value="props.modelValue.show_hidden"
      @update:model-value="(value: boolean) => patch({ show_hidden: value })"
    />
  </div>
</template>
