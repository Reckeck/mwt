<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ChevronDown, CircleAlert, Pencil, Receipt, Trash2, TrendingDown, TrendingUp } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { MeterReadingForm } from '@/features/meter-reading'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { UtilityDialog } from '@/features/utility-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { notify } from '@/shared/lib/notify'

import UtilityHistory from './UtilityHistory.vue'
import { useStoreTree } from '@/shared/api/queries'

const props = defineProps<{ item: UtilityItem }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const trend = useQuery({
  // no placeholder: another item's trend
  queryKey: computed(() => ['utility-trend', props.item.id]),
  queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: props.item.id }),
})
const tree = useStoreTree()

const kindName = computed(() =>
  'metered' in props.item.kind ? 'metered' : 'static' in props.item.kind ? 'static' : 'housing',
)
const last = computed(() => trend.data.value?.months.at(-1) ?? null)

/* --- «Записать расход»: the payment form, pre-filled --- */
const draft = ref<UtilityPaymentDraft | null>(null)
const payOpen = ref(false)
/** The bill's wallet, else the first budget wallet. */
const payStore = computed(() => {
  const own = draft.value && 'payment' in draft.value.transaction ? draft.value.transaction.payment.store_id : ''
  if (own) return own
  const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
  return (nodes.find((node) => node.store.purpose === 'everyday') ?? nodes[0])?.store.id ?? ''
})
const prepare = useMutation({
  mutationFn: () => call<UtilityPaymentDraft>('get_utility_payment_draft', { itemId: props.item.id, period: null }),
  onSuccess: (result) => {
    draft.value = result
    payOpen.value = true
  },
  onError: (err) => notify.error(err, 'utilities.draftFailed'),
  meta: { silent: 'loads the bill draft; the payment dialog that opens is the result' },
})

const editOpen = ref(false)
const historyOpen = ref(false)
const remove = useMutation({
  mutationFn: () => call('delete_utility_item', { id: props.item.id }),
  onSuccess: () => {
    for (const queryKey of [['utilities'], ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'utilities.deleteFailed', success: 'utilities.deleted' },
})

function onPaid() {
  void queryClient.invalidateQueries({ queryKey: ['utility-trend'] })
}
</script>

<template>
  <Card :class="!props.item.active && 'opacity-60'">
    <div class="grid gap-3">
      <div class="flex items-start justify-between gap-2">
        <div class="grid min-w-0 gap-1">
          <div class="flex flex-wrap items-center gap-2">
            <p class="truncate font-medium">{{ props.item.name }}</p>
            <Badge variant="outline">{{ t(`utilities.kindName.${kindName}`) }}</Badge>
            <Badge v-if="trend.data.value?.anomaly" variant="destructive" class="gap-1">
              <CircleAlert class="size-3" />
              {{ t('utilities.anomaly') }}
            </Badge>
          </div>
          <p class="text-sm tabular-nums">
            <i18n-t keypath="utilities.monthly">
              <template #amount><Money :amount="trend.data.value?.monthly" :currency="props.item.currency" /></template>
            </i18n-t>
            <span
              v-if="trend.data.value?.delta_pct"
              class="ml-1 inline-flex items-center gap-0.5 text-xs"
              :class="Number(trend.data.value.delta_pct) > 0 ? 'text-destructive' : 'text-success'"
            >
              <component :is="Number(trend.data.value.delta_pct) > 0 ? TrendingUp : TrendingDown" class="size-3" />
              {{ trend.data.value.delta_pct }}%
            </span>
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            class="text-muted-foreground"
            :title="t('utilities.edit')"
            :aria-label="t('utilities.edit')"
            @click="editOpen = true"
          >
            <Pencil class="size-4" />
          </Button>
          <ConfirmButton :label="t('utilities.delete')" :confirm-label="t('actions.confirmDeleteNamed', { name: props.item.name })" @confirm="remove.mutate()">
            <Trash2 class="size-4" />
          </ConfirmButton>
        </div>
      </div>

      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
        <template v-if="kindName === 'metered' && last">
          <dt class="text-muted-foreground">{{ t('utilities.lastReading') }}</dt>
          <dd class="text-right">{{ last.reading }} · {{ last.period }}</dd>
          <template v-if="last.consumption">
            <dt class="text-muted-foreground">{{ t('utilities.consumption') }}</dt>
            <dd class="text-right">
              {{ last.consumption }} {{ 'metered' in props.item.kind ? props.item.kind.metered.unit : '' }}
            </dd>
          </template>
        </template>
        <template v-if="trend.data.value?.average">
          <dt class="text-muted-foreground">{{ t('utilities.average') }}</dt>
          <dd class="text-right"><Money :amount="trend.data.value.average" :currency="props.item.currency" /></dd>
        </template>
        <template v-if="trend.data.value?.forecast">
          <dt class="text-muted-foreground">{{ t('utilities.forecast') }}</dt>
          <dd class="text-right"><Money :amount="trend.data.value.forecast" :currency="props.item.currency" /></dd>
        </template>
        <template v-if="'housing' in props.item.kind">
          <dt class="text-muted-foreground">{{ t('utilities.dueDay') }}</dt>
          <dd class="text-right">{{ props.item.kind.housing.due_day }}</dd>
        </template>
      </dl>

      <MeterReadingForm v-if="kindName === 'metered'" :item="props.item" />

      <Button variant="outline" size="sm" class="w-fit" :disabled="prepare.isPending.value" @click="prepare.mutate()">
        <Receipt class="size-4" />
        {{ t('utilities.recordExpense') }}
      </Button>
      <p v-if="draft?.includes_deposit" class="text-xs text-muted-foreground">{{ t('utilities.depositIncluded') }}</p>

      <Button
        variant="ghost"
        size="block"
        class="w-fit px-1 py-0.5 text-xs flex w-fit items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
        :aria-expanded="historyOpen"
        @click="historyOpen = !historyOpen"
      >
        <ChevronDown class="size-3.5 transition-transform" :class="historyOpen && 'rotate-180'" />
        {{ t('utilityHistory.title') }}
      </Button>
      <UtilityHistory v-if="historyOpen" :item="props.item" :months="trend.data.value?.months ?? []" />
    </div>

    <UtilityDialog v-model:open="editOpen" :item="props.item" />
    <TransactionCreateDialog
      v-if="draft && payStore"
      v-model:open="payOpen"
      :store-id="payStore"
      :prefill="draft.transaction"
      @update:open="(isOpen) => !isOpen && onPaid()"
    />
  </Card>
</template>
