<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Dialog } from '@/shared/ui/dialog'
import { converterFrom, converterOpen } from '@/shared/lib/converter'

import ConverterPanel from './ui/ConverterPanel.vue'

/** The converter on its own, from the command palette or a wallet's header. */
const { t } = useI18n()
</script>

<template>
  <Dialog v-model:open="converterOpen" :title="t('rates.converter')" size="sm">
    <ConverterPanel v-if="converterOpen" :from="converterFrom" />
  </Dialog>
</template>
