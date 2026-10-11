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
import { Input } from '@/shared/ui/input'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { call, parseError } from '@/shared/api/ipc'
import { activeLocale } from '@/shared/lib/locale'
import { LEDGER_KEYS, readingDate, readingDateParts } from '@/shared/lib/planning'
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
/** The reading's date; moving it to another month moves the reading there. */
const draftDate = ref('')
const draftParts = computed(() => readingDateParts(draftDate.value))
function startEdit(reading: MeterReading) {
  editing.value = reading.id
  draft.value = reading.value
  draftDate.value = readingDate(reading)
}
const saveReading = useMutation({
  mutationFn: (reading: MeterReading) =>
    call<MeterReading>('update_meter_reading', {
      input: {
        id: reading.id,
        value: draft.value,
        period: draftParts.value?.period ?? reading.period,
        day: draftParts.value?.day ?? reading.day,
      } satisfies UpdateMeterReadingInput,
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
        <p v-if="month.consumption" class="text-xs text-muted-foreground tabular-nums">
          {{ t('utilities.consumption') }}: {{ month.consumption }} {{ unit }}
        </p>
        <div
          v-for="reading in month.readings"
          :key="reading.id"
          class="flex items-center gap-2 text-xs text-muted-foreground"
        >
          <template v-if="editing === reading.id">
            <Input v-model="draftDate" type="date" class="h-7 w-36 px-2" :aria-label="t('utilities.day')" />
            <AmountInput v-model="draft" class="h-7 w-32" :aria-label="t('utilities.reading')" />
            <Button
              variant="ghost"
              size="icon"
              class="size-7"
              :aria-label="t('actions.save')"
              :disabled="!draft || !draftParts || saveReading.isPending.value"
              @click="saveReading.mutate(reading)"
            >
              <Check class="size-3.5" />
            </Button>
            <Button variant="ghost" size="icon" class="size-7" :aria-label="t('actions.cancel')" @click="editing = null">
              <X class="size-3.5" />
            </Button>
          </template>
          <template v-else>
            <span class="tabular-nums">
              {{ reading.day ? t('utilities.readingOnDay', { day: reading.day }) : t('utilities.readingAtEnd') }}:
              {{ reading.value }} {{ unit }}
            </span>
            <Button
              variant="ghost"
              size="icon"
              class="size-6"
              :aria-label="t('utilityHistory.editReading')"
              @click="startEdit(reading)"
            >
              <Pencil class="size-3" />
            </Button>
            <ConfirmButton
              :label="t('utilityHistory.deleteReading')"
              :confirm-label="t('actions.confirmDelete')"
              class="size-6"
              @confirm="removeReading.mutate(reading.id)"
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
