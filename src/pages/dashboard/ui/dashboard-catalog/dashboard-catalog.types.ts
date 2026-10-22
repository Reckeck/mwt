export interface DashboardCatalogExchange {
  id: string
  name: string
}

export interface DashboardCatalogProps {
  /** Ids already on Home: their cards are pressed. */
  placed: string[]
  /** Exchanges a trading widget can show; without any, trading widgets are not offered. */
  exchanges: DashboardCatalogExchange[]
}
