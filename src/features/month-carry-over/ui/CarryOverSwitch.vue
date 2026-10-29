<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { MONTH_BALANCE_KEY } from '@/entities/report'
import { call } from '@/shared/api/ipc'
import { Label } from '@/shared/ui/label'
import { Switch } from '@/shared/ui/switch'

/**
 * `Carry what is left into the next month`: what a month left joins the
 * next one. Turned on, the carry starts with `from` (the first month the user
 * sees), so the table shows its effect at once; off, each month stands alone.
 */
const props = defineProps<{ carrySince: string | null; from: string }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const carry = useMutation({
  mutationFn: (month: string | null) => call<null>('set_carry_over', { month }),
  onSuccess: () => queryClient.invalidateQueries({ queryKey: [...MONTH_BALANCE_KEY] }),
  meta: { error: 'carry.saveFailed', success: (_data, month) => (month ? t('carry.on') : t('carry.off')) },
})
const on = computed({
  get: () => (carry.isPending.value ? carry.variables.value !== null : props.carrySince !== null),
  set: (value: boolean) => carry.mutate(value ? props.from : null),
})
</script>

<template>
  <div class="flex items-center gap-2" data-tour="carry-over">
    <Switch id="carry-over" v-model="on" :disabled="carry.isPending.value" />
    <Label for="carry-over" class="text-sm font-normal">{{ t('carry.label') }}</Label>
  </div>
</template>
