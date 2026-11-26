<script setup lang="ts">
import { ref } from 'vue'

import type { PadKey } from '../../lib/keypad-text'
import { AccountRefPicker } from '../account-ref-picker'
import { QuickAmount } from '../quick-amount'

/**
 * One side of an exchange (boards 462813, 460801): a rounded box with the wallet and a caption
 * under its name on the left, the amount and its currency on the right. The side being typed has a
 * dark ring, not a thicker border (rule 17). On a phone the owner's keypad types the active side
 * through the exposed `press`.
 */
const props = defineProps<{
  storeId: string
  amountId: string
  caption: string | ((balance: string) => string)
  currency: string
  label: string
  active: boolean
  storeInvalid?: boolean
  amountInvalid?: boolean
}>()
const emit = defineEmits<{ activate: [] }>()
const wallet = defineModel<string>('wallet', { default: '' })
const amount = defineModel<string>('amount', { default: '' })

const field = ref<InstanceType<typeof QuickAmount> | null>(null)
defineExpose({ press: (key: PadKey) => field.value?.press(key) })
</script>

<template>
  <div
    class="flex min-h-16 items-center gap-2 rounded-xl border px-2 transition-shadow"
    :class="[props.active ? 'border-foreground ring-1 ring-foreground' : 'border-border', props.amountInvalid && 'border-destructive']"
    data-exchange-side
    :data-active="props.active || undefined"
    @focusin="emit('activate')"
    @pointerdown="emit('activate')"
  >
    <AccountRefPicker :id="props.storeId" v-model="wallet" variant="inline" :caption="props.caption" :invalid="props.storeInvalid" class="max-w-[60%]" />
    <QuickAmount
      ref="field"
      :id="props.amountId"
      v-model="amount"
      size="inline"
      class="min-w-0 flex-1"
      :invalid="props.amountInvalid"
      :aria-label="props.label"
    >
      <template v-if="props.currency" #suffix>{{ props.currency }}</template>
    </QuickAmount>
  </div>
</template>
