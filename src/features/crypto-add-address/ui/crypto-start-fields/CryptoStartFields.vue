<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Label } from '@/shared/ui/label'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { formatDay } from '@/shared/lib/format'

/** Step 3 (mockup 10): from which date the history counts, or all of it. */
const props = defineProps<{ error: string | undefined }>()
const whole = defineModel<boolean>('whole', { required: true })
const date = defineModel<string>('date', { required: true })
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-4">
    <RadioGroup :model-value="whole ? 'whole' : 'from'" class="gap-3" @update:model-value="(value) => (whole = value === 'whole')">
      <div class="flex items-start gap-3">
        <RadioGroupItem id="crypto-start-from" value="from" class="mt-0.5" />
        <Label for="crypto-start-from" class="flex flex-col items-start gap-0.5 font-normal">
          <span class="font-medium">{{ t('cryptoTrack.connect.fromDate') }}</span>
          <span class="text-xs text-muted-foreground">{{ t('cryptoTrack.connect.fromDateHint') }}</span>
        </Label>
      </div>
      <div class="flex items-start gap-3">
        <RadioGroupItem id="crypto-start-whole" value="whole" class="mt-0.5" />
        <Label for="crypto-start-whole" class="flex flex-col items-start gap-0.5 font-normal">
          <span class="font-medium">{{ t('cryptoTrack.connect.wholeHistory') }}</span>
          <span class="text-xs text-muted-foreground">{{ t('cryptoTrack.connect.wholeHistoryHint') }}</span>
        </Label>
      </div>
    </RadioGroup>

    <!-- The field keeps its place when the whole history is chosen: nothing below jumps. -->
    <FormField for="crypto-start-date" :label="t('cryptoTrack.connect.startDate')" required :error="props.error" :hint="t('cryptoTrack.connect.startDateHint')">
      <DateTimePicker id="crypto-start-date" v-model="date" :disabled="whole" />
    </FormField>

    <p class="rounded-lg bg-muted p-3 text-sm text-muted-foreground">
      {{ whole ? t('cryptoTrack.connect.wholeNote') : t('cryptoTrack.connect.fromNote', { date: formatDay(`${date}T00:00:00Z`) }) }}
    </p>
  </div>
</template>
