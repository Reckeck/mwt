<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CryptoTxList, cryptoOpFilterCount, useCryptoOps } from '@/entities/crypto-tx'
import { CryptoActionsHost, useCryptoActions, useDismissSuggestion } from '@/features/crypto-tx-override'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import type { CryptoOperationsProps } from './crypto-operations.types'
import { useCryptoOpRoute } from './model/use-crypto-op-route'
import CryptoOpFilterBar from './ui/CryptoOpFilterBar.vue'
import CryptoOpSheet from './ui/CryptoOpSheet.vue'
import ExchangeBanner from './ui/ExchangeBanner.vue'

/**
 * The operations the module read from its sources: the wallet's `Operations` tab and the operations page.
 * It is the app's own day-grouped list, not a table. There is nothing here that adds or deletes an
 * operation — they come from the network and are rewritten by every refresh; the menu only corrects what
 * they mean. The filter and the open card are in the address.
 */
const props = withDefaults(defineProps<CryptoOperationsProps>(), { accountIds: () => [], bare: false })
const { t } = useI18n()

const route = useCryptoOpRoute(() => props.accountIds)
const { query, rows, total, facets, suggestion } = useCryptoOps(route.filter)
const menu = useCryptoActions()
const dismiss = useDismissSuggestion()

const narrowed = computed(() => cryptoOpFilterCount(route.filter.value) > 0)
const suggested = computed(() => (narrowed.value ? undefined : suggestion.value))
const suggestedRow = computed(() => rows.value.find((row) => row.id === suggested.value?.event_id))

function linkSuggested() {
  if (suggestedRow.value) menu.askExchange(suggestedRow.value)
}
function dismissSuggested() {
  if (suggested.value) dismiss.mutate({ id: suggested.value.event_id })
}
</script>

<template>
  <div class="grid gap-4" data-tour="crypto-operations">
    <ExchangeBanner v-if="suggested && suggestedRow" :suggestion="suggested" :busy="dismiss.isPending.value" @link="linkSuggested" @dismiss="dismissSuggested" />
    <Card>
      <template #header>
        <CryptoOpFilterBar :model-value="route.filter.value" :facets="facets" :total="total" @update:model-value="route.setFilter" />
      </template>
      <SectionState
        :loading="query.isPending.value"
        :error="query.error.value"
        :empty="!rows.length"
        :empty-title="narrowed ? t('cryptoTrack.operations.emptyFiltered') : t('cryptoTrack.operations.empty')"
        :empty-description="narrowed ? t('cryptoTrack.operations.emptyFilteredHint') : t('cryptoTrack.operations.emptyHint')"
        compact
        @retry="query.refetch()"
      >
        <div class="-my-3">
          <CryptoTxList
            :rows="rows"
            :total="total"
            :loading-more="query.isFetchingNextPage.value"
            :row-actions="menu.actions"
            @open="route.open($event.id)"
            @more="query.fetchNextPage()"
          />
        </div>
      </SectionState>
    </Card>
    <p class="text-sm text-muted-foreground">{{ t('cryptoTrack.operations.readonlyNote') }}</p>
    <CryptoOpSheet :id="route.openId.value" :menu="menu" @close="route.close" />
    <CryptoActionsHost :menu="menu" />
  </div>
</template>
