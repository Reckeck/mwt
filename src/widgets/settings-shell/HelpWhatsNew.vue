<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'
import { WHATS_NEW } from '@/shared/config/whats-new'

/** «Что нового» in a block: the titles of the latest updates; the dialog holds the points. */
const { t } = useI18n()
const router = useRouter()
const latest = computed(() => WHATS_NEW.slice(0, 3))
</script>

<template>
  <SettingsSection :title="t('whatsNew.title')">
    <ul class="grid gap-1 text-sm">
      <li v-for="entry in latest" :key="entry.id">• {{ t(`whatsNew.${entry.id}.title`) }}</li>
    </ul>
    <Button variant="outline" class="w-fit" @click="router.push({ query: { whatsNew: '1' } })">
      {{ t('help.allChanges') }}
    </Button>
  </SettingsSection>
</template>
