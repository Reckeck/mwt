<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CalendarDays } from 'lucide-vue-next'

import { FormField } from '@/shared/ui/form-field'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { TimeInput } from '@/shared/ui/time-input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { formatDayMonthShort, formatWeekday } from '@/shared/lib/format'
import { WEEKDAYS } from '@/shared/lib/planning'

import { WIZARD_FREQUENCIES, WORKDAY_SHIFTS, type WizardFrequency } from '../../automation-edit.constants'
import { useAutomationWizard } from '../../model/use-automation-wizard'

/**
 * Step 3 (Auto-1 · 8): how often, on which day and at what time; where a day
 * off moves it; whether it asks «да» first. Under them, the next three dates
 * as the app will fire them.
 */
const { t } = useI18n()
const { frequency, day, weekday, time, workdayShift, asks, dates } = useAutomationWizard()

const DAYS = Array.from({ length: 31 }, (_, index) => String(index + 1))
const upcoming = computed(() =>
  (dates.data.value ?? []).map((iso) => `${formatDayMonthShort(iso)} (${formatWeekday(new Date(iso))})`).join(', '),
)
const setFrequency = (value: unknown) => {
  if (WIZARD_FREQUENCIES.includes(value as WizardFrequency)) frequency.value = value as WizardFrequency
}
const setShift = (value: unknown) => {
  if (WORKDAY_SHIFTS.includes(value as WorkdayShift)) workdayShift.value = value as WorkdayShift
}
</script>

<template>
  <div class="@container grid gap-4" data-wizard-when>
    <div class="grid gap-2">
      <Label>{{ t('automationWizard.frequency') }}</Label>
      <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-full md:w-fit" :model-value="frequency" @update:model-value="setFrequency">
        <ToggleGroupItem v-for="entry in WIZARD_FREQUENCIES" :key="entry" :value="entry" class="px-3" :data-wizard-frequency="entry">
          {{ t(`automationWizard.often.${entry}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <div class="grid items-start gap-3 @md:grid-cols-2">
      <FormField v-if="frequency === 'monthly'" for="automation-wizard-day" :label="t('automationWizard.day')" required>
        <Select id="automation-wizard-day" v-model="day">
          <SelectItem v-for="entry in DAYS" :key="entry" :value="entry">{{ t('automationWizard.dayOption', { day: entry }) }}</SelectItem>
        </Select>
      </FormField>
      <FormField v-else-if="frequency === 'weekly'" for="automation-wizard-weekday" :label="t('schedule.weekdayLabel')" required>
        <Select id="automation-wizard-weekday" v-model="weekday">
          <SelectItem v-for="entry in WEEKDAYS" :key="entry" :value="String(entry)">{{ t(`schedule.weekday.${entry}`) }}</SelectItem>
        </Select>
      </FormField>
      <FormField for="automation-wizard-time" :label="t('automationWizard.time')">
        <TimeInput id="automation-wizard-time" v-model="time" />
      </FormField>
    </div>

    <!-- Every single day has nothing to move to. -->
    <div v-if="frequency !== 'daily'" class="grid gap-2">
      <Label>{{ t('automationWizard.dayOff') }}</Label>
      <ToggleGroup type="single" variant="segmented" :spacing="1" class="w-full md:w-fit" :model-value="workdayShift" @update:model-value="setShift">
        <ToggleGroupItem v-for="entry in WORKDAY_SHIFTS" :key="entry" :value="entry" class="px-3" :data-wizard-shift="entry">
          {{ t(`automationWizard.shift.${entry}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>

    <SwitchRow id="automation-wizard-asks" v-model="asks" plain :label="t('automationWizard.asks')" :hint="t('automationWizard.asksHint')" />

    <p v-if="upcoming" class="flex items-center gap-2 rounded-[10px] bg-muted px-3 py-2.5 text-sm" data-wizard-dates>
      <CalendarDays class="size-4 shrink-0 text-muted-foreground" />
      <span class="min-w-0 flex-1">{{ t('automationWizard.upcoming', { dates: upcoming }) }}</span>
    </p>
  </div>
</template>
