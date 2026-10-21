<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { firstError, integerInRange, required, useFormValidation } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { useMileageUpdate } from '../model/use-vehicle'

/** The odometer read today: the service by mileage is reminded from it. */
const props = defineProps<{ store: Store }>()
const open = defineModel<boolean>('open', { required: true })
const { t } = useI18n()

const km = ref('')
watch(open, () => (km.value = String(props.store.property?.car?.mileage_km ?? '')), { immediate: true })

const form = useFormValidation(() => ({ km: firstError(required(km.value), integerInRange(km.value, 0, 5_000_000)) }), {
  resetOn: open,
})
const save = useMileageUpdate()

function submit() {
  save.mutate({ store_id: props.store.id, km: Number(km.value.trim()) }, { onSuccess: () => (open.value = false) })
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    :title="t('car.updateMileage')"
    form-id="mileage-form"
    :pending="save.isPending.value"
    @submit="form.submit(submit, $event)"
  >
    <FormField for="mileage-km" :label="t('car.mileage')" required :error="form.error('km')">
      <Input id="mileage-km" v-model="km" inputmode="numeric" />
    </FormField>
  </FormDialog>
</template>
