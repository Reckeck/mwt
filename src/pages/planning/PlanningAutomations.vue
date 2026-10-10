<script setup lang="ts">
import { AutomationList } from '@/widgets/automation-list'
import { UpcomingList } from '@/widgets/upcoming-list'
</script>

<template>
  <div class="grid items-start gap-6 @4xl:grid-cols-[1fr_22rem]">
    <AutomationList />
    <UpcomingList :days="45" :limit="12" />
  </div>
</template>
