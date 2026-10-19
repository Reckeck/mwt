<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Minus, Plus } from 'lucide-vue-next'

import { PointsDialog } from '@/features/loyalty-edit'
import { Money } from '@/entities/currency'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatQuantity } from '@/shared/lib/format'

/** A bonus programme's page: points, what they are worth, what burns next. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()

const report = useQuery({ queryKey: ['cashback', 'year'], queryFn: () => call<CashbackReport>('get_cashback_report', { period: 'year' }) })
const row = computed(() => report.data.value?.programmes.find((entry) => entry.store_id === props.store.id) ?? null)

const dialog = ref<'credit' | 'spend' | null>(null)
const dialogOpen = computed({
  get: () => dialog.value !== null,
  set: (value: boolean) => {
    if (!value) dialog.value = null
  },
})
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('walletSettings.loyalty') }}</p>
    </template>
    <div v-if="!props.store.loyalty" class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-muted-foreground">{{ t('loyalty.setup') }}</p>
      <RouterLink
        :to="{ name: 'wallet-settings', params: { id: props.store.id }, query: { tab: 'loyalty' } }"
        :class="buttonVariants({ variant: 'outline', size: 'sm' })"
      >
        {{ t('walletSettings.loyalty') }}
      </RouterLink>
    </div>
    <div v-else class="flex flex-wrap items-end justify-between gap-4">
      <div class="grid gap-1">
        <p class="text-xs text-muted-foreground">{{ t('loyalty.points') }}</p>
        <p class="text-2xl font-semibold tabular-nums">
          {{ formatQuantity(row?.points ?? '0', 0) }}
          <span class="text-sm font-normal text-muted-foreground">{{ props.store.loyalty.points_symbol }}</span>
        </p>
        <p v-if="row" class="text-sm text-muted-foreground">
          {{ t('loyalty.worth') }} <Money :amount="row.value_base" :currency="report.data.value?.base_currency" />
        </p>
        <p v-if="row?.expires_at" class="text-xs text-warning">
          {{ t('loyalty.expiring', { points: row.expiring_points ?? '', date: formatDate(row.expires_at) }) }}
        </p>
      </div>
      <div class="flex gap-2">
        <Button size="sm" @click="dialog = 'credit'"><Plus class="size-4" />{{ t('loyalty.credit') }}</Button>
        <Button size="sm" variant="outline" @click="dialog = 'spend'"><Minus class="size-4" />{{ t('loyalty.spend') }}</Button>
      </div>
    </div>
    <PointsDialog v-if="dialog" v-model:open="dialogOpen" :store="props.store" :mode="dialog" />
  </Card>
</template>
