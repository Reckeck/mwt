export interface DashboardCatalogExchange {
  id: string
  name: string
}

export interface DashboardCatalogProps {
  /** Ids already on the dashboard; the catalog offers the rest. */
  placed: string[]
  /** Exchanges a trading widget can show; without any, trading widgets are not offered. */
  exchanges: DashboardCatalogExchange[]
}
