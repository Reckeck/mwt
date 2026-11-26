import { computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { WORKFLOW_KEYS, catalogMap } from '@/shared/lib/workflow'

/** The node catalog from Rust; it never changes while the app runs. */
export function useNodeCatalog() {
  const query = useQuery({
    queryKey: WORKFLOW_KEYS.catalog,
    queryFn: () => call<NodeDef[]>('workflow_node_catalog'),
    staleTime: Number.POSITIVE_INFINITY,
  })
  const catalog = computed(() => query.data.value ?? [])
  const defs = computed(() => catalogMap(catalog.value))
  return { query, catalog, defs, def: (kind: string) => defs.value.get(kind) }
}
