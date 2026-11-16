<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { useExchangeSync } from '@/features/exchange-sync'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'

/**
 * Bybit on the dashboard: what the last sync brought and a sync on demand.
 * Nothing waits for confirmation any more — a record becomes an operation of
 * the crypto section at once (spec E7).
 */
const props = defineProps<{ storeId: string }>()
const { t } = useI18n()
const sync = useExchangeSync()

const runs = useQuery({
  // no placeholder: another exchange's runs
  queryKey: computed(() => ['import-runs', props.storeId]),
  queryFn: () => call<ImportRun[]>('list_import_runs', { storeId: props.storeId }),
})
const last = computed(() => runs.data.value?.[0] ?? null)
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="font-medium">{{ t('bybitSync.title') }}</p>
        <Button size="sm" variant="outline" :disabled="sync.syncNow.isPending.value" @click="sync.syncNow.mutate(props.storeId)">
          <RefreshCw class="size-4" :class="sync.syncNow.isPending.value && 'animate-spin'" />
          {{ t('import.syncNow') }}
        </Button>
      </div>
    </template>
    <div class="grid gap-3">
      <p class="text-xs text-muted-foreground">
        <template v-if="!last">{{ t('bybitSync.never') }}</template>
        <template v-else-if="last.error">
          <span class="text-destructive">{{ t('bybitSync.lastFailed', { when: formatDateTime(last.at) }) }}: {{ t(`import.notice.${last.error}`) }}</span>
        </template>
        <template v-else>
          {{ t('cryptoTrack.legacy.lastSync', { when: formatDateTime(last.at), recorded: last.confirmed }) }}
        </template>
      </p>
      <RouterLink
        :to="{ name: 'wallet-settings', params: { id: props.storeId }, query: { tab: 'imported' } }"
        class="justify-self-start text-xs text-primary underline-offset-4 hover:underline"
      >
        {{ t('bybitSync.all') }}
      </RouterLink>
    </div>
  </Card>
</template>
