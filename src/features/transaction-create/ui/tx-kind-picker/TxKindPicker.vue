<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useTxForm } from '../../model/use-tx-form'

/** The kinds the wallet offers, and where a trade is booked when not on this account. */
const { t } = useI18n()
const { form, offered, pickKind, verbLabel, tradeOnUnified, unifiedAccount } = useTxForm()
</script>

<template>
  <ToggleGroup
    v-if="offered.length > 1"
    type="single"
    variant="outline"
    size="sm"
    class="flex-wrap justify-start"
    :spacing="1"
    :model-value="form.kind"
    :aria-label="t('form.kind')"
    @update:model-value="pickKind"
  >
    <ToggleGroupItem v-for="entry in offered" :key="entry" :value="entry">{{ verbLabel(entry) }}</ToggleGroupItem>
  </ToggleGroup>

  <p v-if="tradeOnUnified && unifiedAccount" class="text-xs text-muted-foreground">
    {{ t('tx.tradeOnUnified', { name: unifiedAccount.name }) }}
  </p>
</template>
