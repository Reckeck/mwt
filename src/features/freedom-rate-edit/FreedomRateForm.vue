<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { useFreedomRate } from './model/use-freedom-rate'

/** «Ожидаемая доходность» of the freedom potential; blank falls back to the key rate. */
const props = defineProps<{ rate: Amount | null; keyRate: Amount | null }>()

const { t } = useI18n()
const { draft, form, save, submit } = useFreedomRate(() => props.rate)
</script>

<template>
  <form class="flex items-start gap-2" novalidate @submit.prevent="form.submit(submit)">
    <FormField
      for="freedom-rate"
      class="min-w-0 flex-1"
      :label="t('health.freedom.rate')"
      :hint="t('health.freedom.rateHint')"
      :error="form.error('rate')"
    >
      <Input
        id="freedom-rate"
        v-model="draft"
        type="text"
        inputmode="decimal"
        :placeholder="props.keyRate ? t('health.freedom.keyRate', { rate: props.keyRate }) : ''"
      />
    </FormField>
    <Button type="submit" variant="outline" class="mt-6" :loading="save.isPending.value">{{ t('actions.save') }}</Button>
  </form>
</template>
