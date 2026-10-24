<script setup lang="ts">
import { ref } from 'vue'

import { TemplateGallery } from '@/features/workflow-template-gallery'
import { AutomationList } from '@/widgets/automation-list'
import { UpcomingList } from '@/widgets/upcoming-list'

/** The gallery waits for the list: cards landing above would push it down. */
const listReady = ref(false)
</script>

<template>
  <!-- Every automation in one place: created and edited here (the dialog picks the wallet)
       or on the wallet's own tab, which shows the same list filtered to it. -->
  <div class="grid gap-6">
    <div class="grid grid-cols-[minmax(0,1fr)] items-start gap-4 @4xl:grid-cols-[minmax(0,1fr)_22rem]">
      <AutomationList @ready="listReady = true" />
      <UpcomingList :days="45" :limit="12" />
    </div>
    <TemplateGallery v-if="listReady" />
  </div>
</template>
