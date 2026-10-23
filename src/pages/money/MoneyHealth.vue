<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { CapitalHealth } from '@/widgets/capital-health'
import { EmergencyFundDialog } from '@/features/emergency-fund-edit'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SectionState } from '@/shared/ui/section-state'

import { useHealth } from './model/use-health'
import { HealthFigures } from './ui/health'

/**
 * The checkup (Money 3 · 16): the main thing in a sentence and three figures.
 * The full checkup (reasons, accounts, passive income) waits below, folded.
 */
const { t } = useI18n()
const report = useHealth()
const details = ref(false)
const fundOpen = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState :loading="report.isPending.value" :error="report.error.value" skeleton="chart" @retry="report.refetch()">
      <HealthFigures v-if="report.data.value" :report="report.data.value" @setup-fund="fundOpen = true" />
    </SectionState>
    <Collapsible v-model:open="details" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-health-details>
          {{ t('money.health.details') }} <ChevronDown :class="['transition-transform', details && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <CapitalHealth v-if="details" />
      </CollapsibleContent>
    </Collapsible>
    <EmergencyFundDialog
      v-if="report.data.value"
      v-model:open="fundOpen"
      :fund="report.data.value.emergency.fund"
      :status="report.data.value.emergency"
    />
  </div>
</template>
