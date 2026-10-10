<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { CircleAlert, Pencil, Receipt, Trash2, TrendingDown, TrendingUp } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { MeterReadingForm } from '@/features/meter-reading'
import { TransactionCreateDialog } from '@/features/transaction-create'
import { UtilityDialog } from '@/features/utility-edit'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

const props = defineProps<{ item: UtilityItem }>()

const { t } = useI18n()
const queryClient = useQueryClient()

const trend = useQuery({
  queryKey: computed(() => ['utility-trend', props.item.id]),
  queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: props.item.id }),
})
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

const kindName = computed(() =>
  'metered' in props.item.kind ? 'metered' : 'static' in props.item.kind ? 'static' : 'housing',
)
const money = (value: string | null | undefined) => formatMoney(value, { currency: props.item.currency })
const last = computed(() => trend.data.value?.months.at(-1) ?? null)

/* --- «Записать расход»: the payment form, pre-filled --- */
const draft = ref<UtilityPaymentDraft | null>(null)
const payOpen = ref(false)
/** The bill's wallet, else the first budget wallet. */
const payStore = computed(() => {
  const own = draft.value && 'payment' in draft.value.transaction ? draft.value.transaction.payment.store_id : ''
  if (own) return own
  const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
  return (nodes.find((node) => node.store.in_budget) ?? nodes[0])?.store.id ?? ''
})
const prepare = useMutation({
  mutationFn: () => call<UtilityPaymentDraft>('get_utility_payment_draft', { itemId: props.item.id, period: null }),
  onSuccess: (result) => {
    draft.value = result
    payOpen.value = true
  },
  onError: (err) => toast.error(t('utilities.draftFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

const editOpen = ref(false)
const remove = useMutation({
  mutationFn: () => call('delete_utility_item', { id: props.item.id }),
  onSuccess: () => {
    toast.success(t('utilities.deleted'))
    for (const queryKey of [['utilities'], ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('utilities.deleteFailed'), { description: codeErrorMessage(parseError(err), t) }),
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
            {{ t('utilities.monthly', { amount: money(trend.data.value?.monthly) }) }}
            <span
              v-if="trend.data.value?.delta_pct"
              class="ml-1 inline-flex items-center gap-0.5 text-xs"
              :class="Number(trend.data.value.delta_pct) > 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400'"
            >
              <component :is="Number(trend.data.value.delta_pct) > 0 ? TrendingUp : TrendingDown" class="size-3" />
              {{ trend.data.value.delta_pct }}%
            </span>
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            class="size-8 text-muted-foreground"
            :title="t('utilities.edit')"
            :aria-label="t('utilities.edit')"
            @click="editOpen = true"
          >
            <Pencil class="size-4" />
          </Button>
          <ConfirmButton :label="t('utilities.delete')" :confirm-label="t('actions.confirmDelete')" @confirm="remove.mutate()">
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
          <dd class="text-right">{{ money(trend.data.value.average) }}</dd>
        </template>
        <template v-if="trend.data.value?.forecast">
          <dt class="text-muted-foreground">{{ t('utilities.forecast') }}</dt>
          <dd class="text-right">{{ money(trend.data.value.forecast) }}</dd>
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
