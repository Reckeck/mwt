<script setup lang="ts">
import { FlowList } from '@/widgets/flow-list'
import { LossSummary } from '@/widgets/loss-summary'
</script>

<template>
  <div class="grid gap-6">
    <LossSummary />
    <FlowList />
  </div>
</template>
