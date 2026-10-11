<script setup lang="ts">
import { AutomationList } from '@/widgets/automation-list'
import { UpcomingList } from '@/widgets/upcoming-list'
</script>

<template>
  <!-- An overview: automations are created on their wallet, here they are only watched. -->
  <div class="grid grid-cols-[minmax(0,1fr)] items-start gap-4 @4xl:grid-cols-[minmax(0,1fr)_22rem]">
    <AutomationList readonly />
    <UpcomingList :days="45" :limit="12" />
  </div>
</template>
