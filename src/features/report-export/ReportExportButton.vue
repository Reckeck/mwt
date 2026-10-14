<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { Download } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { call, isTauri } from '@/shared/api/ipc'

import type { ReportExportButtonProps } from './report-export.types'

const props = defineProps<ReportExportButtonProps>()

const { t } = useI18n()

/** The save dialog and the file write happen in Rust: the webview cannot download. */
const exportCsv = useMutation({
  mutationFn: () => call<string | null>('export_report_csv', { kind: props.kind, filter: props.filter }),
  meta: {
    error: 'report.exportFailed',
    success: (data) => (data ? { text: t('report.exported'), description: data as string } : ''),
  },
})
</script>

<template>
  <Button
    variant="outline"
    size="sm"
    :disabled="!isTauri || exportCsv.isPending.value"
    :title="isTauri ? undefined : t('report.exportDesktopOnly')"
    @click="exportCsv.mutate()"
  >
    <Download class="size-4" />
    {{ t('report.exportCsv') }}
  </Button>
</template>
