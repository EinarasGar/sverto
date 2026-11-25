import { startTransition, useMemo, type ReactNode } from "react"
import { useSuspenseQuery } from "@tanstack/react-query"

import { useBaseCurrency, useUserId } from "@/auth"
import type { ChartMarker, ChartPeriod, ChartPoint } from "@/components/chart"
import { HeroChart, HeroChartSkeleton } from "@/components/chart"
import { useSearchState } from "@/components/layout/search-state"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import { assetConvertedRatesQueryOptions } from "@/features/assets/api"
import type { AssetHolding } from "@/features/portfolio/api"
import {
  historyChartPoints,
  usePortfolioHistorySuspense,
  useRequiredBaseAssetId,
} from "@/features/portfolio/api"
import { formatMoney } from "@/lib/format"

import {
  readAssetChartSeries,
  writeAssetChartSeries,
  type AssetChartSeries,
} from "./asset-series"
import { AssetSeriesSwitch } from "./asset-series-switch"

const CHART_EMPTY_LABEL =
  "No price history for this window — Sverto has no rates for this pair"

const HOLDING_EMPTY_LABEL = "No holding history for this window yet"

const SERIES_LABELS: Record<AssetChartSeries, string> = {
  price: "Unit price",
  value: "Value of your holdings",
  units: "Units you hold",
}

const SERIES_LOADING: Record<AssetChartSeries, string> = {
  price: "Loading the price line",
  value: "Loading the value of your holdings",
  units: "Loading the units you hold",
}

function markerNote(lotCount: number, hasAverage: boolean): string {
  const lots = lotCount === 1 ? "purchase lot" : "purchase lots"
  const average = hasAverage
    ? ", and the dashed line is your average cost across every account"
    : ""
  return `The rings are your ${String(lotCount)} ${lots}, drawn at the unit price you paid${average}.`
}

function AssetPriceChart({
  assetId,
  holding,
  period,
  onPeriodChange,
  controls,
}: {
  assetId: number
  holding: AssetHolding
  period: ChartPeriod
  onPeriodChange: (next: ChartPeriod) => void
  controls: ReactNode
}) {
  const defaultAssetId = useRequiredBaseAssetId()
  const baseCurrency = useBaseCurrency()
  const { data: rates } = useSuspenseQuery(
    assetConvertedRatesQueryOptions(assetId, defaultAssetId, period)
  )

  const points = useMemo<ChartPoint[]>(
    () => rates.map((point) => ({ date: point.date, value: point.rate })),
    [rates]
  )
  const markers = useMemo<ChartMarker[]>(
    () =>
      holding.lots.map((lot) => ({
        date: lot.addedAt,
        value: lot.addPrice,
      })),
    [holding.lots]
  )

  const average = holding.averageUnitCost

  return (
    <HeroChart
      data={points}
      currency={baseCurrency}
      label={SERIES_LABELS.price}
      controls={controls}
      size="tall"
      period={period}
      onPeriodChange={onPeriodChange}
      markers={markers}
      emptyLabel={CHART_EMPTY_LABEL}
      {...(average === null
        ? {}
        : {
            referenceLine: {
              value: average,
              label: `avg cost ${formatMoney(average, { currency: baseCurrency })}`,
            },
          })}
      footer={
        <p className="text-[11px] leading-[1.4] text-pretty text-ink-3">
          {markerNote(holding.lots.length, average !== null)}
        </p>
      }
    />
  )
}

function AssetHoldingChart({
  assetId,
  series,
  period,
  onPeriodChange,
  controls,
}: {
  assetId: number
  series: Exclude<AssetChartSeries, "price">
  period: ChartPeriod
  onPeriodChange: (next: ChartPeriod) => void
  controls: ReactNode
}) {
  const userId = useUserId()
  const baseAssetId = useRequiredBaseAssetId()
  const baseCurrency = useBaseCurrency()
  const history = usePortfolioHistorySuspense({
    userId,
    assetId,
    defaultAssetId: series === "units" ? assetId : baseAssetId,
    range: period,
  })
  const points = useMemo(() => historyChartPoints(history), [history])

  const shared = {
    data: points,
    label: SERIES_LABELS[series],
    controls,
    size: "tall",
    period,
    onPeriodChange,
    emptyLabel: HOLDING_EMPTY_LABEL,
    showChangeRatio: false,
  } as const

  return series === "units" ? (
    <HeroChart {...shared} kind="units" />
  ) : (
    <HeroChart {...shared} currency={baseCurrency} />
  )
}

export function AssetHero({
  assetId,
  holding,
  period,
  onPeriodChange,
}: {
  assetId: number
  holding: AssetHolding
  period: ChartPeriod
  onPeriodChange: (next: ChartPeriod) => void
}) {
  const [series, setSeries] = useSearchState(
    "chart",
    readAssetChartSeries,
    writeAssetChartSeries
  )
  const controls = (
    <AssetSeriesSwitch
      value={series}
      onValueChange={(next) => {
        startTransition(() => {
          setSeries(next)
        })
      }}
    />
  )

  return (
    <PanelBoundary
      pending={
        <HeroChartSkeleton
          size="tall"
          label={SERIES_LOADING[series]}
          controls={controls}
          period={period}
          onPeriodChange={onPeriodChange}
        />
      }
    >
      {series === "price" ? (
        <AssetPriceChart
          assetId={assetId}
          holding={holding}
          period={period}
          onPeriodChange={onPeriodChange}
          controls={controls}
        />
      ) : (
        <AssetHoldingChart
          assetId={assetId}
          series={series}
          period={period}
          onPeriodChange={onPeriodChange}
          controls={controls}
        />
      )}
    </PanelBoundary>
  )
}
