<script setup lang="ts">
import type { CryptoActions } from '../model/use-crypto-actions'

import CryptoExchangeDialog from './CryptoExchangeDialog.vue'
import CryptoMeaningDialog from './CryptoMeaningDialog.vue'

/** The dialogs the menu of an operation opens; mounted once beside the list. */
const props = defineProps<{ menu: CryptoActions }>()
</script>

<template>
  <CryptoExchangeDialog v-if="props.menu.exchangeFor.value" v-model:open="props.menu.exchangeOpen.value" :row="props.menu.exchangeFor.value" />
  <CryptoMeaningDialog
    v-if="props.menu.meaningFor.value"
    v-model:open="props.menu.meaningOpen.value"
    :row="props.menu.meaningFor.value"
    :focus="props.menu.meaningFocus.value"
  />
</template>
