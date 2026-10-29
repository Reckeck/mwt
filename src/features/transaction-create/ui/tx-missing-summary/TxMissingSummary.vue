<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { useTxForm } from '../../model/use-tx-form'

/** `To save, fill in…`: the long form's summary of what is still missing. */
const { t } = useI18n()
const { missing, check } = useTxForm()
</script>

<template>
  <p v-if="missing.length" class="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground" data-tour="tx-missing">
    {{ t('tx.missing.title') }}: {{ missing.map((key) => t(`tx.missing.${key}`)).join(', ') }}
  </p>
  <p v-if="!missing.length && check.error('form')" class="text-xs text-destructive" role="alert">{{ check.error('form') }}</p>
</template>
