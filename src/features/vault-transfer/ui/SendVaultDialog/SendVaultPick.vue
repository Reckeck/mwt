<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Laptop, Smartphone } from 'lucide-vue-next'

import { parseError } from '@/shared/api/ipc'
import { required, useFormValidation } from '@/shared/lib/form'
import { cn } from '@/shared/lib/utils'
import { vaultErrorMessage } from '@/shared/lib/vault-error'
import { EmptyState } from '@/shared/ui/empty-state'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { Spinner } from '@/shared/ui/spinner'

/** Step one: the device to hand the vault to, and the vault password again. */
const props = defineProps<{ nearby: TransferNearby[]; pending: boolean; failure: unknown }>()
const emit = defineEmits<{ submit: [input: TransferSendInput] }>()
const { t } = useI18n()

const picked = ref('')
const password = ref('')
// The backend's refusal belongs to the password it was given for: typing
// another one clears it at once.
const rejected = ref<{ password: string; message: string } | null>(null)
let tried = ''

// A device that left the list cannot be picked any more.
watch(
  () => props.nearby,
  (list) => {
    if (picked.value && !list.some((device) => device.id === picked.value)) picked.value = ''
  },
)
watch(
  () => props.failure,
  (err) => {
    rejected.value = err
      ? {
          password: tried,
          message: parseError(err).code === 'SECRET' ? t('transfer.send.wrongPassword') : vaultErrorMessage(err, t),
        }
      : null
  },
)

const form = useFormValidation(() => ({
  device: required(picked.value, 'pick'),
  password:
    required(password.value) ?? (rejected.value?.password === password.value ? rejected.value.message : null),
}))
const deviceError = computed(() => form.error('device'))

function submit(event: Event) {
  form.submit(() => {
    tried = password.value
    emit('submit', { id: picked.value, password: password.value })
  }, event)
}
</script>

<template>
  <form id="transfer-send-form" class="grid gap-4" @submit.prevent="submit">
    <div class="grid gap-2" :data-invalid="form.invalidAttr('device')">
      <p class="text-sm font-medium">
        {{ t('transfer.send.devices') }} <span class="text-destructive">*</span>
      </p>
      <EmptyState v-if="!nearby.length" compact :title="t('transfer.send.searching')" :description="t('transfer.send.searchingHint')">
        <Spinner />
      </EmptyState>
      <RadioGroup v-else v-model="picked" class="grid gap-1.5" :aria-label="t('transfer.send.devices')">
        <label
          v-for="device in nearby"
          :key="device.id"
          :for="`transfer-device-${device.id}`"
          :class="
            cn(
              'flex cursor-pointer items-center gap-3 rounded-md border border-border px-3 py-2 text-sm transition-colors hover:bg-accent has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring',
              picked === device.id && 'border-primary bg-primary/5',
            )
          "
        >
          <RadioGroupItem :id="`transfer-device-${device.id}`" :value="device.id" />
          <Smartphone v-if="device.kind === 'mobile'" class="size-4 shrink-0 text-muted-foreground" />
          <Laptop v-else class="size-4 shrink-0 text-muted-foreground" />
          <span class="truncate font-medium">{{ device.name }}</span>
        </label>
      </RadioGroup>
      <p v-if="deviceError" class="text-xs text-destructive">{{ deviceError }}</p>
    </div>

    <FormField for="transfer-password" :label="t('transfer.send.password')" required :hint="t('transfer.send.passwordHint')" :error="form.error('password')">
      <Input id="transfer-password" v-model="password" type="password" autocomplete="current-password" :disabled="pending" />
    </FormField>

    <p class="text-xs text-warning">{{ t('transfer.send.warning') }}</p>
  </form>
</template>
