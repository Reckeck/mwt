<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { absolute, reassignTargets, sumDecimals, useSettleRemainder } from '@/entities/shared-event'
import { formatList, formatMoney } from '@/shared/lib/format'
import { FormDialog } from '@/shared/ui/form-dialog'

import type { SharedRemainderDialogProps } from '../shared-remainder.types'

/**
 * `Write off the remainder` and `Move onto the others`: asks once, with the sum, because both change
 * the shares for good. The save toast offers `Undo`.
 */
const props = defineProps<SharedRemainderDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const mutation = useSettleRemainder()

const name = computed(() => props.person?.name || t('sharedEvents.me'))
const currency = computed(() => props.event.event.currency)
const decimals = computed(() => sumDecimals(props.event))
const rest = computed(() => absolute(props.person?.saldo ?? '0'))
const restText = computed(() => formatMoney(rest.value, { currency: currency.value, decimals: decimals.value }))
const others = computed(() =>
  props.person ? reassignTargets(props.event, props.person).map((person) => person.name || t('sharedEvents.me')) : [],
)

function save() {
  if (!props.person) return
  mutation.mutate(
    {
      kind: props.kind,
      // Empty `amount`: the whole rest; empty `to`: everybody else.
      input: { event_id: props.event.event.id, participant_id: props.person.id, amount: null, to: [] },
      name: name.value,
      amount: rest.value,
      currency: currency.value,
      decimals: decimals.value,
    },
    { onSuccess: () => (open.value = false) },
  )
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-remainder"
    form-id="shared-remainder-form"
    size="sm"
    :title="t(`sharedEvents.remainder.${props.kind}.title`, { amount: restText })"
    :description="t(`sharedEvents.remainder.${props.kind}.description`, { name })"
    :submit-label="t(`sharedEvents.remainder.${props.kind}.submit`)"
    :pending="mutation.isPending.value"
    phone-submit-only
    @submit="save"
  >
    <i18n-t :keypath="`sharedEvents.remainder.${props.kind}.effect`" tag="p" class="text-sm">
      <template #amount>
        <Money :amount="rest" :currency="currency" :decimals="decimals" :convert="false" />
      </template>
      <template #names>{{ formatList(others) }}</template>
    </i18n-t>
  </FormDialog>
</template>
