<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { Download } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { call, isTauri, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { ReportExportButtonProps } from './report-export.types'

const props = defineProps<ReportExportButtonProps>()

const { t } = useI18n()

/** The save dialog and the file write happen in Rust: the webview cannot download. */
const exportCsv = useMutation({
  mutationFn: () => call<string | null>('export_report_csv', { kind: props.kind, filter: props.filter }),
  onSuccess: (path) => {
    if (path) toast.success(t('report.exported'), { description: path })
  },
  onError: (err) => toast.error(t('report.exportFailed'), { description: codeErrorMessage(parseError(err), t) }),
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
