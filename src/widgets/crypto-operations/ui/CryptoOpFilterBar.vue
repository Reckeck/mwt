<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { cryptoOpFilterCount } from '@/entities/crypto-tx'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { FilterBar } from '@/shared/ui/filter-bar'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'

/**
 * The filter of the operations: the address is the quick field, network, token, protocol and kind sit under
 * `More filters` and apply together. The value lives in the address of the page (the owner writes it).
 */
const props = defineProps<{ modelValue: CryptoOpFilter; facets: CryptoOpFacets; total: number }>()
const emit = defineEmits<{ 'update:modelValue': [CryptoOpFilter] }>()
const { t } = useI18n()

const ALL = 'all'
const KINDS: CryptoTxKind[] = ['transfer', 'swap', 'approve', 'stake', 'unstake', 'claim', 'bridge', 'contract', 'unknown']
const open = ref(false)
const draft = ref<CryptoOpFilter>({ ...props.modelValue })
watch(
  () => props.modelValue,
  (next) => {
    draft.value = { ...next }
  },
)
const address = computed({
  get: () => props.modelValue.address ?? '',
  set: (value: string) => emit('update:modelValue', { ...props.modelValue, address: value.trim() || undefined }),
})
const pick = (field: 'chain' | 'token' | 'protocol' | 'kind') => ({
  get: () => (draft.value[field] as string | undefined) ?? ALL,
  set: (value: string) => {
    draft.value = { ...draft.value, [field]: value === ALL ? undefined : value }
  },
})
const chain = computed(pick('chain'))
const token = computed(pick('token'))
const protocol = computed(pick('protocol'))
const kind = computed(pick('kind'))
const morePicked = computed(() => cryptoOpFilterCount({ ...props.modelValue, address: undefined }))
function apply() {
  emit('update:modelValue', { ...draft.value, address: props.modelValue.address })
}
function clear() {
  draft.value = { account_ids: props.modelValue.account_ids, show_hidden: false, address: props.modelValue.address }
}
function reset() {
  emit('update:modelValue', { account_ids: props.modelValue.account_ids, show_hidden: false })
}
</script>

<template>
  <FilterBar
    v-model:open="open"
    :active="cryptoOpFilterCount(props.modelValue)"
    :more-active="morePicked"
    :apply-label="t('cryptoTrack.operations.filter.show', { count: props.total })"
    @apply="apply"
    @clear="clear"
    @reset="reset"
  >
    <template #lead>
      <div class="relative min-w-0 flex-1 basis-56 @container">
        <Search class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
        <Input
          v-model="address"
          class="pl-9"
          :aria-label="t('cryptoTrack.operations.filter.address')"
          :placeholder="t('cryptoTrack.operations.filter.address')"
          data-crypto-filter-address
        />
      </div>
    </template>
    <template #more>
      <FormField :label="t('cryptoTrack.operations.filter.network')" for="crypto-filter-chain">
        <Select id="crypto-filter-chain" v-model="chain">
          <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.any') }}</SelectItem>
          <SelectItem v-for="item in props.facets.chains" :key="item" :value="item">{{ CHAIN_LABELS[item] }}</SelectItem>
        </Select>
      </FormField>
      <FormField :label="t('cryptoTrack.operations.filter.token')" for="crypto-filter-token">
        <Select id="crypto-filter-token" v-model="token">
          <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.any') }}</SelectItem>
          <SelectItem v-for="item in props.facets.tokens" :key="item.key" :value="item.key">{{ item.symbol }}</SelectItem>
        </Select>
      </FormField>
      <FormField :label="t('cryptoTrack.operations.filter.protocol')" for="crypto-filter-protocol">
        <Select id="crypto-filter-protocol" v-model="protocol">
          <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.any') }}</SelectItem>
          <SelectItem v-for="item in props.facets.protocols" :key="item" :value="item">{{ item }}</SelectItem>
        </Select>
      </FormField>
      <FormField :label="t('cryptoTrack.operations.filter.kind')" for="crypto-filter-kind">
        <Select id="crypto-filter-kind" v-model="kind">
          <SelectItem :value="ALL">{{ t('cryptoTrack.operations.filter.any') }}</SelectItem>
          <SelectItem v-for="item in KINDS" :key="item" :value="item">{{ t(`cryptoTrack.operations.kind.${item}`) }}</SelectItem>
        </Select>
      </FormField>
    </template>
    <template #switches>
      <SwitchRow
        id="crypto-filter-hidden"
        plain
        :label="t('cryptoTrack.operations.filter.hidden')"
        :model-value="draft.show_hidden"
        @update:model-value="(value: boolean) => (draft = { ...draft, show_hidden: value })"
      />
    </template>
  </FilterBar>
</template>
