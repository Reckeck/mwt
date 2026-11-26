<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { walletRow } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { useSettingsStore } from '@/shared/store/settings'
import { Select, SelectItem } from '@/shared/ui/select'

import { quickWalletNodes } from '../../lib/quick-wallets'

/**
 * The wallet of a quick operation: a list of the wallets an operation can be written on (a
 * sub-account, never a group that has live ones). The full picker with search is T26's.
 */
const props = defineProps<{ invalid?: boolean; placeholder?: string }>()
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()

const rows = computed(() =>
  quickWalletNodes(tree.data.value).map((node) => walletRow(node, t, settings.baseCurrency)),
)
</script>

<template>
  <Select v-model="model" :invalid="props.invalid" :placeholder="props.placeholder" :aria-label="props.placeholder">
    <SelectItem v-for="row in rows" :key="row.id" :value="row.id">{{ row.title }}</SelectItem>
  </Select>
</template>
