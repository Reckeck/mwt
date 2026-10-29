<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { SwitchRow } from '@/shared/ui/switch-row'

import { useEmergencyFund } from '../model/use-emergency-fund'

const props = defineProps<{
  store: Store
  /** The purpose picked in the form, saved or not: leaving `Savings` takes the wallet out on save. */
  purpose: Purpose
}>()
const { t } = useI18n()

const { inFund, toggle } = useEmergencyFund(() => props.store.id)
</script>

<template>
  <div class="grid gap-1.5" data-tour="wallet-emergency">
    <SwitchRow
      id="store-emergency"
      :label="t('emergency.switch')"
      :hint="t('emergency.switchHint')"
      :model-value="inFund"
      :disabled="toggle.isPending.value || store.archived"
      @update:model-value="(on: boolean) => toggle.mutate(on)"
    />
    <p v-if="inFund && purpose !== 'savings'" class="text-xs text-warning" role="status">
      {{ t('emergency.leavesOnSave') }}
    </p>
  </div>
</template>
