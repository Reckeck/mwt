<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import type { CryptoOperationsProps } from './crypto-operations.types'
import { cryptoOpFilterCount } from './model/crypto-op-filter'
import { useCryptoOpRoute } from './model/use-crypto-op-route'
import { useCryptoOps } from './model/use-crypto-ops'
import CryptoOpFilterBar from './ui/CryptoOpFilterBar.vue'
import CryptoOpList from './ui/CryptoOpList.vue'

/**
 * The operations the module read from its sources. There is nothing here that adds or deletes an
 * operation: they come from the network and are rewritten by every refresh. The filter is in the address.
 */
const props = withDefaults(defineProps<CryptoOperationsProps>(), { accountIds: () => [] })
const { t } = useI18n()

const route = useCryptoOpRoute(() => props.accountIds)
const { query, rows, total, facets } = useCryptoOps(route.filter)
const narrowed = computed(() => cryptoOpFilterCount(route.filter.value) > 0)
</script>

<template>
  <div class="grid gap-4" data-tour="crypto-operations">
    <CryptoOpFilterBar :model-value="route.filter.value" :facets="facets" @update:model-value="route.setFilter" />
    <Card>
      <SectionState
        skeleton="operations"
        :loading="query.isPending.value"
        :error="query.error.value"
        :empty="!rows.length"
        :empty-title="narrowed ? t('cryptoTrack.operations.emptyFiltered') : t('cryptoTrack.operations.empty')"
        :empty-description="narrowed ? t('cryptoTrack.operations.emptyFilteredHint') : t('cryptoTrack.operations.emptyHint')"
        compact
        @retry="query.refetch()"
      >
        <CryptoOpList :rows="rows" :total="total" :loading-more="query.isFetchingNextPage.value" @more="query.fetchNextPage()" />
      </SectionState>
    </Card>
    <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.operations.readonlyNote') }}</p>
  </div>
</template>
