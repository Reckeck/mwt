<script setup lang="ts">
import { AutomationList } from '@/widgets/automation-list'
import { UpcomingList } from '@/widgets/upcoming-list'
</script>

<template>
  <!-- Every automation in one place: created and edited here (the dialog picks the wallet)
       or on the wallet's own tab, which shows the same list filtered to it. -->
  <div class="grid grid-cols-[minmax(0,1fr)] items-start gap-4 @4xl:grid-cols-[minmax(0,1fr)_22rem]">
    <AutomationList />
    <UpcomingList :days="45" :limit="12" />
  </div>
</template>
