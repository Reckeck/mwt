<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Bell, Pencil, Plus, Trash2 } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { PriceAlertDialog } from '@/features/price-alert-edit'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { ListRow } from '@/shared/ui/list-row'
import { SectionState } from '@/shared/ui/section-state'

/** Price alerts: asset, direction, level, and when it last fired. */
const { t } = useI18n()
const queryClient = useQueryClient()
const alerts = useQuery({
  queryKey: ['price-alerts'],
  queryFn: () => call<PriceAlert[]>('list_price_alerts'),
  placeholderData: keepPreviousData,
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_price_alert', { id }),
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'alert.deleteFailed', success: 'alert.deleted' },
})

const dialogOpen = ref(false)
const editing = ref<PriceAlert | null>(null)
function openDialog(alert: PriceAlert | null) {
  editing.value = alert
  dialogOpen.value = true
}
</script>

<template>
  <div class="grid gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('alert.intro') }}</p>
      <Button size="sm" data-tour="alerts-add" @click="openDialog(null)"><Plus class="size-4" />{{ t('alert.new') }}</Button>
    </div>
    <SectionState
      :loading="alerts.isPending.value"
      :error="alerts.error.value"
      :empty="!alerts.data.value?.length"
      :empty-title="t('alert.empty')"
      :empty-description="t('alert.emptyHint')"
      :rows="3"
      @retry="alerts.refetch()"
    >
      <Card size="sm">
        <ul class="divide-y divide-border">
          <li v-for="alert in alerts.data.value" :key="alert.id" class="flex items-center gap-2" :class="!alert.active && 'opacity-60'" :data-alert="alert.id">
            <div class="min-w-0 flex-1">
              <ListRow
                :title="`${alert.asset.symbol} ${alert.condition === 'above' ? t('alert.above') : t('alert.below')}`"
                :subtitle="alert.triggered_at ? t('alert.triggered', { at: formatDateTime(alert.triggered_at) }) : undefined"
                :icon="Bell"
              >
                <template #value><Money :amount="alert.price" :currency="alert.currency" /></template>
              </ListRow>
            </div>
            <Badge v-if="!alert.active" variant="outline">{{ t('alert.paused') }}</Badge>
            <Button variant="ghost" size="icon-sm" class="text-muted-foreground" :aria-label="t('alert.edit')" @click="openDialog(alert)">
              <Pencil class="size-4" />
            </Button>
            <ConfirmButton :label="t('alert.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(alert.id)">
              <Trash2 class="size-4" />
            </ConfirmButton>
          </li>
        </ul>
      </Card>
    </SectionState>
    <PriceAlertDialog v-model:open="dialogOpen" :alert="editing" />
  </div>
</template>
