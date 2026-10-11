<script setup lang="ts">
import { AutomationList } from '@/widgets/automation-list'
import { UpcomingList } from '@/widgets/upcoming-list'
</script>

<template>
  <!-- An overview: automations are created on their wallet, here they are only watched. -->
  <div class="grid items-start gap-4 @4xl:grid-cols-[1fr_22rem]">
    <AutomationList readonly />
    <UpcomingList :days="45" :limit="12" />
  </div>
</template>
