<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Pencil, Plus, Trash2 } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { PriceAlertDialog, type PriceAlertPreset } from '@/features/price-alert-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { EmptyState } from '@/shared/ui/empty-state'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call } from '@/shared/api/ipc'
import { formatDateTime, } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'

/**
 * With `storeId`, only that exchange's alerts are listed and new ones belong to it.
 * With `asset`, only the alerts on that ticker; `preset` fills a new one.
 */
const props = withDefaults(
  defineProps<{ storeId?: string | null; asset?: AssetRef | null; preset?: PriceAlertPreset | null }>(),
  { storeId: null, asset: null, preset: null },
)
const { t } = useI18n()
const queryClient = useQueryClient()

const alerts = useQuery({ queryKey: ['price-alerts'], queryFn: () => call<PriceAlert[]>('list_price_alerts') })
const shown = computed(() =>
  (alerts.data.value ?? []).filter(
    (alert) =>
      (!props.storeId || alert.store_id === props.storeId) &&
      (!props.asset || alert.asset.symbol.toUpperCase() === props.asset.symbol.toUpperCase()),
  ),
)

const remove = useMutation({
  mutationFn: (id: string) => call('delete_price_alert', { id }),
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'alert.deleteFailed', success: 'alert.deleted' },
})

const dialogOpen = ref(false)
const editing = ref<PriceAlert | null>(null)
function openNew() {
  editing.value = null
  dialogOpen.value = true
}
function openEdit(alert: PriceAlert) {
  editing.value = alert
  dialogOpen.value = true
}
</script>

<template>
  <div class="grid gap-4">
    <ListHeader
      :intro="props.storeId ? t('alert.title') : t('alert.intro')"
      :action-label="t('alert.new')"
      :hide-action="alerts.isSuccess.value && !shown.length"
      @action="openNew"
    />

    <EmptyState v-if="alerts.isSuccess.value && !shown.length" compact :title="t('alert.empty')" :description="t('alert.emptyHint')">
      <Button @click="openNew"><Plus class="size-4" />{{ t('alert.new') }}</Button>
    </EmptyState>

    <ul class="grid gap-1">
      <li
        v-for="alert in shown"
        :key="alert.id"
        class="flex flex-wrap items-center gap-3 rounded-md px-3 py-2 hover:bg-accent/50"
        :class="!alert.active && 'opacity-60'"
      >
        <span class="min-w-0 flex-1 text-sm">
          <span class="font-medium">{{ alert.asset.symbol }}</span>
          {{ alert.condition === 'above' ? t('alert.above') : t('alert.below') }}
          <span class="tabular-nums"><Money :amount="alert.price" :currency="alert.currency" /></span>
        </span>
        <Badge v-if="alert.triggered_at" variant="secondary">
          {{ t('alert.triggered', { at: formatDateTime(alert.triggered_at) }) }}
        </Badge>
        <Badge v-else-if="!alert.active" variant="outline">{{ t('alert.paused') }}</Badge>
        <Button
          variant="ghost"
          size="icon"
          class="size-8 text-muted-foreground"
          :title="t('alert.edit')"
          :aria-label="t('alert.edit')"
          @click="openEdit(alert)"
        >
          <Pencil class="size-4" />
        </Button>
        <ConfirmButton :label="t('alert.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate(alert.id)">
          <Trash2 class="size-4" />
        </ConfirmButton>
      </li>
    </ul>

    <PriceAlertDialog v-model:open="dialogOpen" :alert="editing" :store-id="props.storeId" :preset="editing ? null : props.preset" />
  </div>
</template>
