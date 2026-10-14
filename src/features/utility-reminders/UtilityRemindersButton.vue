<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { Bell, BellOff } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'

import { useUtilityReminders } from './use-utility-reminders'

/**
 * «Remind» on the utilities tab: sets the day the property's utilities are
 * paid, which turns on the automatic reminders to pay them and to send the
 * meter readings. Until now that day hid in the wallet settings.
 */
const props = defineProps<{ store: Store }>()

const { t } = useI18n()
const { setDay, turnOff } = useUtilityReminders()

const open = ref(false)
const day = ref('')
const current = computed(() => props.store.property?.utilities_pay_day ?? null)
watch(open, (isOpen) => {
  if (isOpen) day.value = current.value ? String(current.value) : ''
})

const form = useFormValidation(() => ({
  day: firstError(required(day.value), integerInRange(day.value, 1, 31)),
}))

function save() {
  setDay.mutate({ store: props.store, day: Number(day.value) }, { onSuccess: () => (open.value = false) })
}
function switchOff() {
  turnOff.mutate(props.store, { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <Button variant="outline" :disabled="props.store.archived" data-tour="utility-reminders" @click="open = true">
    <Bell class="size-4" />
    {{ current ? t('utilityReminders.buttonOn', { day: current }) : t('utilityReminders.button') }}
  </Button>

  <FormDialog
    v-model:open="open"
    :title="t('utilityReminders.title')"
    :description="t('utilityReminders.description')"
    form-id="utility-reminders-form"
    :submit-label="current ? t('actions.save') : t('utilityReminders.turnOn')"
    :pending="setDay.isPending.value"
    size="sm"
    @submit="form.submit(save, $event)"
  >
    <FormField
      :label="t('utilities.payDay')"
      for="utility-reminders-day"
      required
      :error="form.error('day')"
      :hint="t('utilityReminders.dayHint')"
    >
      <Input
        id="utility-reminders-day"
        v-model="day"
        type="number"
        inputmode="numeric"
        min="1"
        max="31"
        :aria-invalid="form.invalidAttr('day')"
      />
    </FormField>
    <p class="text-sm text-muted-foreground">{{ t('utilityReminders.what') }}</p>
    <div class="flex flex-wrap items-center gap-2">
      <Button variant="link" class="h-auto p-0" as-child>
        <RouterLink :to="{ name: 'planning-reminders' }" @click="open = false">{{ t('utilityReminders.openList') }}</RouterLink>
      </Button>
      <Button
        v-if="current"
        type="button"
        variant="ghost"
        size="sm"
        class="ml-auto text-muted-foreground"
        :disabled="turnOff.isPending.value"
        @click="switchOff"
      >
        <BellOff class="size-4" />
        {{ t('utilityReminders.turnOff') }}
      </Button>
    </div>
  </FormDialog>
</template>
