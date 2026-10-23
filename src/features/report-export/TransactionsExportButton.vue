<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation } from '@tanstack/vue-query'
import { ChevronDown, Download } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { DropdownMenu } from '@/shared/ui/dropdown-menu'
import { DropdownMenuItem } from '@/shared/ui/dropdown-menu-item'
import { call, isTauri } from '@/shared/api/ipc'

import type { TransactionsExportButtonProps } from './report-export.types'

/**
 * «Экспорт ▾» of the operations list: the operations the filter keeps, a line
 * per leg, or the categories × months summary of the same wallets and months.
 */
const props = defineProps<TransactionsExportButtonProps>()

const { t } = useI18n()

/** The summary counts whole months: the filter's days are read as their months. */
const reportFilter = computed<ReportFilter>(() => ({
  scope: props.filter.scope,
  from: props.filter.from?.slice(0, 7) ?? null,
  to: props.filter.to?.slice(0, 7) ?? null,
  year: null,
  currency: null,
  real_terms: false,
}))

/** The save dialog and the file write happen in Rust: the webview cannot download. */
const exportCsv = useMutation({
  mutationFn: (what: 'operations' | 'categories') =>
    what === 'operations'
      ? call<string | null>('export_transactions_csv', { filter: props.filter })
      : call<string | null>('export_report_csv', { kind: 'categories', filter: reportFilter.value }),
  meta: {
    error: 'report.exportFailed',
    success: (data) => (data ? { text: t('report.exported'), description: data as string } : ''),
  },
})
</script>

<template>
  <DropdownMenu align="end">
    <template #trigger>
      <Button variant="outline" class="shrink-0" :disabled="exportCsv.isPending.value">
        <Download class="size-4" />
        {{ t('report.export') }}
        <ChevronDown class="size-4" />
      </Button>
    </template>
    <DropdownMenuItem value="operations" :disabled="!isTauri" @select="exportCsv.mutate('operations')">
      {{ t('report.exportOperations') }}
    </DropdownMenuItem>
    <DropdownMenuItem value="categories" :disabled="!isTauri" @select="exportCsv.mutate('categories')">
      {{ t('report.exportCategories') }}
    </DropdownMenuItem>
    <!-- Outside the desktop app the reason is said in words, not only in a tooltip. -->
    <p v-if="!isTauri" class="max-w-60 px-2 py-1.5 text-xs text-muted-foreground">{{ t('report.exportDesktopOnly') }}</p>
  </DropdownMenu>
</template>
