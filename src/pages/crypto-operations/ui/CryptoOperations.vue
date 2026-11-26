<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CryptoOperations } from '@/widgets/crypto-operations'
import { PageHeader } from '@/shared/ui/page-header'

/** The operations of every followed address in one list. A thin composition: the list is the widget. */
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-4">
    <PageHeader :title="t('cryptoTrack.operations.title')" :description="t('cryptoTrack.operations.pageHint')" />
    <CryptoOperations />
  </div>
</template>
