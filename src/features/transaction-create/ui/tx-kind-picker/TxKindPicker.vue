<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { useTxForm } from '../../model/use-tx-form'

/** The kinds the wallet offers. */
const { t } = useI18n()
const { form, offered, pickKind, verbLabel } = useTxForm()
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

</template>
