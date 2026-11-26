<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { SwitchRow } from '@/shared/ui/switch-row'

/** «As a loan: it comes back later» on a transfer. */
const model = defineModel<boolean>({ required: true })
const { t } = useI18n()
</script>

<template>
  <SwitchRow id="quick-debt" v-model="model" :label="t('tx.quick.asDebt')" data-quick-debt />
</template>
