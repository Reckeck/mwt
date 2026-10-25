<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Dialog } from '@/shared/ui/dialog'

import AppSettingsModal from './AppSettingsModal.vue'

const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
</script>

<template>
  <!-- Settings save as they change, so there is no footer to confirm. -->
  <Dialog overlay="app-settings-unlock" v-model:open="open" :title="t('settings.appTitle')" :description="t('settings.appScopeHint')" size="lg">
    <!-- Mounted per open so every visit starts from the file on disk. -->
    <AppSettingsModal v-if="open" />
  </Dialog>
</template>
