<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Badge } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'

const { t } = useI18n()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})
</script>

<template>
  <Card>
    <template #header>
      <p class="text-sm text-muted-foreground">{{ t('store.assets') }}</p>
    </template>
    <div class="flex flex-wrap gap-1.5">
      <Badge v-for="asset in summary.data.value?.assets ?? []" :key="asset.asset.symbol" variant="secondary">
        {{ asset.asset.symbol }} · {{ asset.quantity }}
      </Badge>
    </div>
  </Card>
</template>
