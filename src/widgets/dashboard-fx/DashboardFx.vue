<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { formatMoney } from '@/shared/lib/format'
import { call, parseError } from '@/shared/api/ipc'

const { t } = useI18n()
const queryClient = useQueryClient()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const refreshBusy = computed(() => summary.isFetching.value)

async function refresh() {
  try {
    const result = await call<RefreshResult>('refresh_all')
    queryClient.setQueryData(['refresh'], result)
    toast.success(t('store.refreshed'), { description: `${result.prices} · ${result.fx}` })
    await queryClient.invalidateQueries({ queryKey: ['summary'] })
  } catch (err) {
    toast.error(t('store.refreshFailed'), { description: parseError(err).message })
  }
}
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between">
        <p class="text-sm text-muted-foreground">{{ t('store.fxRate') }}</p>
        <Button variant="ghost" size="sm" :disabled="refreshBusy" @click="refresh">
          <RefreshCw class="size-4" :class="refreshBusy && 'animate-spin'" />
          {{ t('store.refresh') }}
        </Button>
      </div>
    </template>
    <p class="text-lg font-semibold">
      {{ formatMoney(summary.data.value?.fx_per_usd, { currency: summary.data.value?.base_currency }) }}
    </p>
  </Card>
</template>
