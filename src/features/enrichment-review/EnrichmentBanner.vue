<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { SearchCheck } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { txChainAndHash, txId } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'

import type { EnrichmentBannerProps } from './enrichment-review.types'

const props = defineProps<EnrichmentBannerProps>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const id = computed(() => txId(props.tx))
const hasHash = computed(() => Boolean(txChainAndHash(props.tx).hash))
const records = useQuery({
  queryKey: computed(() => ['enrichments', id.value]),
  queryFn: () => call<EnrichmentRecord[]>('get_enrichments', { txId: id.value }),
  enabled: hasHash,
})
const pending = computed(() => records.data.value?.find((record) => record.status === 'pending') ?? null)

function refresh() {
  void queryClient.invalidateQueries({ queryKey: ['enrichments'] })
  for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
}

const apply = useMutation({
  mutationFn: (recordId: string) => call<EnrichmentRecord>('apply_enrichment', { id: recordId }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'enrichment.applyFailed', success: 'enrichment.applied' },
})
const reject = useMutation({
  mutationFn: (recordId: string) => call<EnrichmentRecord>('reject_enrichment', { id: recordId }),
  onSuccess: refresh,
  meta: { success: 'enrichment.rejected', error: 'enrichment.applyFailed' },
})
const recheck = useMutation({
  mutationFn: () => call<EnrichmentRecord | null>('re_enrich', { txId: id.value }),
  onSuccess: () => {
    void queryClient.invalidateQueries({ queryKey: ['enrichments'] })
  },
  meta: {
    error: 'enrichment.checkFailed',
    success: (data) => (data ? t('enrichment.found') : t('enrichment.nothing')),
  },
})

const feeText = computed(() => {
  const fee = pending.value?.found.fee
  return fee ? `${fee.amount} ${fee.asset.symbol}` : null
})
</script>

<template>
  <div v-if="hasHash" class="grid gap-2">
    <div v-if="pending" class="grid gap-2 rounded-md border border-primary/40 bg-primary/5 p-3 text-sm">
      <p class="flex items-center gap-2 font-medium">
        <SearchCheck class="size-4" />
        {{ t('enrichment.found') }}
      </p>
      <ul class="grid gap-0.5 text-xs text-muted-foreground">
        <li v-if="feeText">{{ t('enrichment.fee', { fee: feeText }) }}</li>
        <li v-if="pending.found.confirmed !== null">
          {{ pending.found.confirmed ? t('enrichment.confirmed') : t('enrichment.unconfirmed') }}
        </li>
        <li v-if="pending.found.block_time">{{ t('enrichment.blockTime', { at: formatDateTime(pending.found.block_time) }) }}</li>
      </ul>
      <p v-if="feeText" class="text-xs text-muted-foreground">
        {{ t('enrichment.applyHint') }}
      </p>
      <div class="flex gap-2">
        <Button size="sm" :disabled="apply.isPending.value" @click="apply.mutate(pending.id)">{{ t('enrichment.apply') }}</Button>
        <Button size="sm" variant="ghost" :disabled="reject.isPending.value" @click="reject.mutate(pending.id)">
          {{ t('enrichment.reject') }}
        </Button>
      </div>
    </div>
    <Button
      v-else-if="settings.app?.enrichment_opt_in"
      variant="outline"
      size="sm"
      class="w-fit"
      :disabled="recheck.isPending.value"
      @click="recheck.mutate()"
    >
      <SearchCheck class="size-4" />
      {{ t('enrichment.check') }}
    </Button>
    <Button
      v-else
      variant="ghost"
      size="sm"
      class="w-fit text-muted-foreground"
      @click="settings.enrichmentConsentOpen = true"
    >
      {{ t('enrichment.enable') }}
    </Button>
  </div>
</template>
