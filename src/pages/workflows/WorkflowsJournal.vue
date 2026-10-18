<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { WorkflowQueue } from '@/widgets/workflow-queue'
import { WorkflowRuns } from '@/widgets/workflow-runs'

const { t } = useI18n()
</script>

<template>
  <!-- What waits for you first, then what already ran. -->
  <div class="grid gap-6">
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('workflow.journal.waiting') }}</h2>
      <WorkflowQueue />
    </section>
    <section class="grid gap-3">
      <h2 class="heading-section">{{ t('workflow.journal.history') }}</h2>
      <WorkflowRuns />
    </section>
  </div>
</template>
