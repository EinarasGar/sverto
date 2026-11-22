import { useMemo } from "react"
import { useSuspenseQuery } from "@tanstack/react-query"

import { useBaseCurrency, useUserId } from "@/auth"
import type { ChartMarker } from "@/components/chart"
import { HeroChart, HeroChartSkeleton } from "@/components/chart"
import { Figure } from "@/components/figure"
import { PanelBoundary } from "@/components/layout/route-boundaries"
import type { RatePoint } from "@/features/assets/api"
import {
  userAssetConvertedRatesQueryOptions,
  userAssetPairRatesQueryOptions,
} from "@/features/assets/api"
import { useRequiredBaseAssetId } from "@/features/portfolio/api"
import { formatDateStamp, formatMoney } from "@/lib/format"

import type { Property } from "./api"
import { valueTrend } from "./value-trend"

function TrendChart({
  valuations,
  currentValue,
}: {
  valuations: readonly RatePoint[]
  currentValue: number | null
}) {
  const baseCurrency = useBaseCurrency()

  const { line, markers } = useMemo(() => {
    const now = new Date().getTime()
    const trend = valueTrend(
      valuations.map((point) => ({
        timestamp: point.date.getTime() / 1000,
        value: point.rate,
      })),
      Math.floor(now / 1000),
      currentValue
    )
    return {
      line: trend.map((point) => ({
        date: point.timestamp * 1000,
        value: point.value,
      })),
      markers: valuations
        .filter((point) => point.date.getTime() <= now)
        .map((point): ChartMarker => ({
          date: point.date,
          value: point.rate,
          label: `Valued ${formatMoney(point.rate, { currency: baseCurrency })} · ${formatDateStamp(point.date, { year: "always" })}`,
        })),
    }
  }, [valuations, currentValue, baseCurrency])

  const first = line[0]?.value
  const last = line[line.length - 1]?.value
  const changeRatio =
    first === undefined || last === undefined || first === 0
      ? null
      : (last - first) / Math.abs(first)

  return (
    <HeroChart
      data={line}
      markers={markers}
      currency={baseCurrency}
      label="Value"
      size="tall"
      emptyLabel="No value recorded yet"
      note={
        <>
          <Figure
            value={changeRatio}
            kind="percent"
            scale="ratio"
            intent="secondary"
            sign="always"
            size="base"
          />
          <span>since purchase</span>
        </>
      }
    />
  )
}

interface ValueChartProps {
  property: Property
  currentValue: number | null
}

function OwnCurrencyChart({ property, currentValue }: ValueChartProps) {
  const userId = useUserId()
  const valuations = useSuspenseQuery(
    userAssetPairRatesQueryOptions(
      userId,
      property.assetId,
      property.currencyAssetId,
      "all"
    )
  ).data
  return <TrendChart valuations={valuations} currentValue={currentValue} />
}

function BaseCurrencyChart({ property, currentValue }: ValueChartProps) {
  const userId = useUserId()
  const baseAssetId = useRequiredBaseAssetId()
  const valuations = useSuspenseQuery(
    userAssetConvertedRatesQueryOptions(
      userId,
      property.assetId,
      baseAssetId,
      "all"
    )
  ).data
  return <TrendChart valuations={valuations} currentValue={currentValue} />
}

export function PropertyValueChart({
  property,
  currentValue,
}: ValueChartProps) {
  const baseAssetId = useRequiredBaseAssetId()
  return (
    <PanelBoundary
      pending={<HeroChartSkeleton size="tall" label="Loading value history" />}
    >
      {property.currencyAssetId === baseAssetId ? (
        <OwnCurrencyChart property={property} currentValue={currentValue} />
      ) : (
        <BaseCurrencyChart property={property} currentValue={currentValue} />
      )}
    </PanelBoundary>
  )
}
