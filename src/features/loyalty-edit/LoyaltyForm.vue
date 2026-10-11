<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

/** A bonus programme: what its points are called, what one is worth, when some burn. */
const props = defineProps<{ store: Store }>()
const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const symbol = ref('')
const value = ref('')
const currency = ref('')
const expiresAt = ref('')
const expiring = ref('')
watch(
  () => props.store,
  (store) => {
    symbol.value = store.loyalty?.points_symbol ?? ''
    value.value = store.loyalty?.point_value ?? '1'
    currency.value = store.loyalty?.currency ?? settings.baseCurrency
    expiresAt.value = store.loyalty?.expires_at ?? ''
    expiring.value = store.loyalty?.expiring_points ?? ''
  },
  { immediate: true },
)

const save = useMutation({
  mutationFn: () =>
    call<Store>('set_loyalty_info', {
      input: {
        store_id: props.store.id,
        info: {
          points_symbol: symbol.value.trim(),
          point_value: value.value || '0',
          currency: currency.value,
          expires_at: expiresAt.value || null,
          expiring_points: expiring.value || null,
        },
      } satisfies SetLoyaltyInfoInput,
    }),
  onSuccess: () => {
    toast.success(t('settings.saved'))
    for (const queryKey of [STORE_TREE_QUERY_KEY, ['cashback'], ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form class="@container grid gap-4" @submit.prevent="symbol.trim() && save.mutate()">
    <div class="grid gap-4 @sm:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="loyalty-symbol">{{ t('loyalty.symbol') }}</Label>
        <Input id="loyalty-symbol" v-model="symbol" class="uppercase" maxlength="16" />
        <p class="text-xs text-muted-foreground">{{ t('loyalty.symbolHint') }}</p>
      </div>
      <div class="grid gap-1.5">
        <Label for="loyalty-value">{{ t('loyalty.pointValue') }}</Label>
        <AmountInput id="loyalty-value" v-model="value" :decimals="4" />
      </div>
      <CurrencyPicker v-model="currency" :label="t('form.currency')" :kinds="['fiat']" />
    </div>
    <div class="grid gap-4 @sm:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="loyalty-expires">{{ t('loyalty.expiresAt') }}</Label>
        <Input id="loyalty-expires" v-model="expiresAt" type="date" />
      </div>
      <div class="grid gap-1.5">
        <Label for="loyalty-expiring">{{ t('loyalty.expiringPoints') }}</Label>
        <AmountInput id="loyalty-expiring" v-model="expiring" />
      </div>
    </div>
    <div>
      <Button type="submit" :disabled="!symbol.trim() || save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
