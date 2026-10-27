<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { ChevronDown, Download } from 'lucide-vue-next'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { call, isTauri } from '@/shared/api/ipc'
import { useIsPhone } from '@/shared/lib/phone'

import type { ReportExportButtonProps } from './report-export.types'

const props = defineProps<ReportExportButtonProps>()

const { t } = useI18n()
const phone = useIsPhone()
/** The phone keeps one «⋯» in the filter row; the export is in it. Outside the app the reason is on the item. */
const actions = computed(() =>
  props.kinds.map((kind) => ({
    id: kind,
    label: props.kinds.length > 1 ? t(`report.exportKind.${kind}`) : t('report.exportCsv'),
    disabled: isTauri ? false : t('report.exportDesktopOnly'),
    run: () => exportCsv.mutate(kind),
  })),
)

/** The save dialog and the file write happen in Rust: the webview cannot download. */
const exportCsv = useMutation({
  mutationFn: (kind: ReportKind) => call<string | null>('export_report_csv', { kind, filter: props.filter }),
  meta: {
    error: 'report.exportFailed',
    success: (data) => (data ? { text: t('report.exported'), description: data as string } : ''),
  },
})
</script>

<template>
  <ActionMenu v-if="phone" :actions="actions" :label="t('report.export')" />
  <!-- Outside the desktop app the reason is in the tooltip of the disabled button. -->
  <span v-else class="inline-flex flex-wrap items-center gap-2">
    <DropdownMenu v-if="props.kinds.length > 1" align="end">
      <template #trigger>
        <Button
          variant="outline"
          size="sm"
          :disabled="!isTauri || exportCsv.isPending.value"
          :title="isTauri ? undefined : t('report.exportDesktopOnly')"
        >
          <Download class="size-4" />
          {{ t('report.exportCsv') }}
          <ChevronDown class="size-4" />
        </Button>
      </template>
      <DropdownMenuItem v-for="kind in props.kinds" :key="kind" :value="kind" @select="exportCsv.mutate(kind)">
        {{ t(`report.exportKind.${kind}`) }}
      </DropdownMenuItem>
    </DropdownMenu>
    <Button
      v-else-if="props.kinds.length"
      variant="outline"
      size="sm"
      :disabled="!isTauri || exportCsv.isPending.value"
      :title="isTauri ? undefined : t('report.exportDesktopOnly')"
      @click="exportCsv.mutate(props.kinds[0])"
    >
      <Download class="size-4" />
      {{ t('report.exportCsv') }}
    </Button>
  </span>
</template>
