<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { CircleAlert, RefreshCw } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { formatDate } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { call, parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table') })

const refreshBusy = computed(() => summary.isFetching.value || table.isFetching.value)
const today = localDate(new Date())

/** No rate for the base currency at all: every total is really in dollars. */
const baseUnpriced = computed(() => summary.data.value?.base_fx_as_of === null)

/** What one unit of USD and of each favourite costs in the base currency. */
const rows = computed(() => {
  const data = table.data.value
  if (!data) return []
  const wanted = [...new Set(['usd', ...settings.favoriteCurrencies.map((code) => code.toLowerCase())])].filter(
    (code) => code !== data.base,
  )
  return wanted.flatMap((code) => {
    const row = data.rows.find((entry) => entry.currency === code)
    if (!row) return []
    const day = row.updated_at.slice(0, 10)
    return [{ code, inBase: row.in_base, stale: day && day < today ? formatDate(row.updated_at) : '' }]
  })
})

async function refresh() {
  try {
    const result = await call<RefreshResult>('refresh_all')
    queryClient.setQueryData(['refresh'], result)
    toast.success(t('store.refreshed'), { description: `${result.prices} · ${result.fx}` })
    await queryClient.invalidateQueries({ queryKey: ['summary'] })
    await queryClient.invalidateQueries({ queryKey: ['fx-table'] })
  } catch (err) {
    toast.error(t('store.refreshFailed'), { description: parseError(err).message })
  }
}
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="text-sm text-muted-foreground">{{ t('store.fxRate') }}</p>
        <!-- An icon button keeps this header as tall as its neighbours'. -->
        <Button
          variant="ghost"
          size="icon"
          class="-my-2 size-8"
          :title="t('store.refresh')"
          :aria-label="t('store.refresh')"
          :disabled="refreshBusy"
          @click="refresh"
        >
          <RefreshCw class="size-4" :class="refreshBusy && 'animate-spin'" />
        </Button>
      </div>
    </template>
    <p v-if="baseUnpriced" class="mb-2 flex items-start gap-1.5 text-xs text-destructive">
      <CircleAlert class="mt-0.5 size-3.5 shrink-0" />
      {{ t('store.fxBaseMissing', { currency: settings.baseCurrency.toUpperCase() }) }}
    </p>
    <p v-if="!rows.length" class="text-sm text-muted-foreground">{{ t('store.fxNoFavorites') }}</p>
    <dl v-else class="grid grid-cols-[auto_1fr_auto] items-baseline gap-x-3 gap-y-1 text-sm">
      <template v-for="row in rows" :key="row.code">
        <dt class="font-medium uppercase">1 {{ row.code }}</dt>
        <dd class="text-right font-semibold"><Money :amount="row.inBase" :convert="false" :decimals="2" /></dd>
        <dd class="text-right text-xs text-muted-foreground">{{ row.stale }}</dd>
      </template>
    </dl>
  </Card>
</template>
