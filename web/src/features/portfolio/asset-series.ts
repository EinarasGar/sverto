export const ASSET_CHART_SERIES = ["price", "value", "units"] as const

export type AssetChartSeries = (typeof ASSET_CHART_SERIES)[number]

export const DEFAULT_ASSET_CHART_SERIES: AssetChartSeries = "value"

export function readAssetChartSeries(
  raw: string | undefined
): AssetChartSeries {
  return (
    ASSET_CHART_SERIES.find((series) => series === raw) ??
    DEFAULT_ASSET_CHART_SERIES
  )
}

export function writeAssetChartSeries(
  series: AssetChartSeries
): string | undefined {
  return series === DEFAULT_ASSET_CHART_SERIES ? undefined : series
}
