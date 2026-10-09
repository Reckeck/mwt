<script setup lang="ts">
import { AllocationPie } from '@/widgets/allocation-pie'
import { PortfolioEvolution } from '@/widgets/portfolio-evolution'
import { provideChartTheme } from '@/shared/lib/echarts'
import { useTheme } from '@/shared/lib/theme'

const { resolved } = useTheme()
provideChartTheme(resolved.value)
</script>

<template>
  <div class="grid gap-6 @container @md:grid-cols-2">
    <PortfolioEvolution />
    <AllocationPie />
  </div>
</template>
