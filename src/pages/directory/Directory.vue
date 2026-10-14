<script setup lang="ts">
import { RouterView } from 'vue-router'
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
