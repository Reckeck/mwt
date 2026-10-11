<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Check, Pencil, Trash2, X } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { TransactionEditDialog } from '@/features/transaction-edit'
import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { activeLocale } from '@/shared/lib/locale'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

/**
 * The last months of one bill: the reading can be corrected or removed in
 * place, and each payment opens in the ordinary edit form.
 */
const props = defineProps<{ item: UtilityItem; months: UtilityMonth[] }>()
const { t } = useI18n()
const queryClient = useQueryClient()

const recent = computed(() => [...props.months].reverse().slice(0, 12))
const unit = computed(() => ('metered' in props.item.kind ? props.item.kind.metered.unit : ''))
const monthLabel = (period: string) =>
  new Intl.DateTimeFormat(activeLocale.value, { month: 'long', year: 'numeric' }).format(new Date(`${period}-01T12:00`))

function refresh() {
  for (const queryKey of [['utility-trend'], ['utility-bill'], ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
}

/* --- correcting a reading --- */
const editing = ref<string | null>(null)
const draft = ref('')
function startEdit(month: UtilityMonth) {
  editing.value = month.reading_id
  draft.value = month.reading ?? ''
}
const saveReading = useMutation({
  mutationFn: (month: UtilityMonth) =>
    call<MeterReading>('update_meter_reading', {
      input: { id: month.reading_id ?? '', value: draft.value, period: month.period } satisfies UpdateMeterReadingInput,
    }),
  onSuccess: () => {
    toast.success(t('utilities.readingSaved'))
    editing.value = null
    refresh()
  },
  onError: (err) => toast.error(t('utilities.readingFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
const removeReading = useMutation({
  mutationFn: (id: string) => call('delete_meter_reading', { id }),
  onSuccess: () => {
    toast.success(t('utilityHistory.readingDeleted'))
    refresh()
  },
  onError: (err) => toast.error(t('utilities.readingFailed'), { description: parseError(err).message }),
})

/* --- editing a payment --- */
const editedTx = ref<Transaction | null>(null)
const txOpen = computed({
  get: () => editedTx.value !== null,
  set: (value: boolean) => {
    if (!value) {
      editedTx.value = null
      refresh()
    }
  },
})
</script>

<template>
  <div class="grid gap-1">
    <p v-if="!recent.length" class="text-xs text-muted-foreground">{{ t('utilityHistory.empty') }}</p>
    <ul v-else class="grid divide-y divide-border text-sm">
      <li v-for="month in recent" :key="month.period" class="grid gap-1 py-1.5">
        <div class="flex items-center justify-between gap-2">
          <span class="capitalize">{{ monthLabel(month.period) }}</span>
          <Money :amount="month.amount" :currency="props.item.currency" :convert="false" class="font-medium" />
        </div>
        <div v-if="month.reading_id" class="flex items-center gap-2 text-xs text-muted-foreground">
          <template v-if="editing === month.reading_id">
            <AmountInput v-model="draft" class="h-7 w-32" :aria-label="t('utilities.reading')" />
            <Button
              variant="ghost"
              size="icon"
              class="size-7"
              :aria-label="t('actions.save')"
              :disabled="!draft || saveReading.isPending.value"
              @click="saveReading.mutate(month)"
            >
              <Check class="size-3.5" />
            </Button>
            <Button variant="ghost" size="icon" class="size-7" :aria-label="t('actions.cancel')" @click="editing = null">
              <X class="size-3.5" />
            </Button>
          </template>
          <template v-else>
            <span class="tabular-nums">
              {{ t('utilities.reading') }}: {{ month.reading }} {{ unit }}
              <template v-if="month.consumption"> · {{ t('utilities.consumption') }} {{ month.consumption }}</template>
            </span>
            <Button
              variant="ghost"
              size="icon"
              class="size-6"
              :aria-label="t('utilityHistory.editReading')"
              @click="startEdit(month)"
            >
              <Pencil class="size-3" />
            </Button>
            <ConfirmButton
              :label="t('utilityHistory.deleteReading')"
              :confirm-label="t('actions.confirmDelete')"
              class="size-6"
              @confirm="removeReading.mutate(month.reading_id)"
            >
              <Trash2 class="size-3" />
            </ConfirmButton>
          </template>
        </div>
        <div v-for="tx in month.payments" :key="'payment' in tx ? tx.payment.meta.id : ''" class="flex items-center gap-2 text-xs">
          <template v-if="'payment' in tx">
            <span class="text-muted-foreground">{{ t('utilityHistory.paid') }}</span>
            <Money :amount="tx.payment.amount" :currency="tx.payment.asset.symbol.toLowerCase()" :convert="false" />
            <Button
              variant="ghost"
              size="icon"
              class="size-6"
              :aria-label="t('utilityHistory.editPayment')"
              @click="editedTx = tx"
            >
              <Pencil class="size-3" />
            </Button>
          </template>
        </div>
      </li>
    </ul>
    <TransactionEditDialog v-if="editedTx" v-model:open="txOpen" :transaction="editedTx" />
  </div>
</template>
