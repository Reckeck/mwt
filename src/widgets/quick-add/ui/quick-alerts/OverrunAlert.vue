<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { TriangleAlert } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'

import type { Overrun } from '../../lib/overrun'

/** The expense would break the category's limit: said before the save, the save stays open. */
const props = defineProps<{ overrun: Overrun }>()
const { t } = useI18n()
</script>

<template>
  <Alert variant="warning" data-quick-overrun>
    <TriangleAlert />
    <AlertTitle>{{ t('quickAdd.overrunTitle') }}</AlertTitle>
    <AlertDescription>
      {{ t('quickAdd.overrunBy') }} <Money :amount="props.overrun.over" :currency="props.overrun.currency" />
    </AlertDescription>
  </Alert>
</template>
