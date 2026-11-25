import { Layers, TrendingUp, Wallet, type LucideIcon } from "lucide-react"

import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"

import { ASSET_CHART_SERIES, type AssetChartSeries } from "./asset-series"

const SERIES_BUTTONS: Record<
  AssetChartSeries,
  { label: string; icon: LucideIcon }
> = {
  price: { label: "Price", icon: TrendingUp },
  value: { label: "Value", icon: Wallet },
  units: { label: "Units", icon: Layers },
}

export function AssetSeriesSwitch({
  value,
  onValueChange,
}: {
  value: AssetChartSeries
  onValueChange: (next: AssetChartSeries) => void
}) {
  return (
    <ToggleGroup
      data-slot="asset-series-switch"
      aria-label="Chart"
      variant="outline"
      size="sm"
      spacing={0}
      value={[value]}
      onValueChange={(next) => {
        const [chosen] = next
        if (chosen !== undefined && chosen !== value) {
          onValueChange(chosen as AssetChartSeries)
        }
      }}
    >
      {ASSET_CHART_SERIES.map((series) => {
        const { label, icon: Icon } = SERIES_BUTTONS[series]
        return (
          <ToggleGroupItem
            key={series}
            value={series}
            aria-label={label}
            className="gap-0 px-2 text-ink-3 aria-pressed:text-ink"
          >
            <Icon aria-hidden />
            {/* The label fades out before its box closes and fades in once it has opened, so no letter is cut mid-way. */}
            <span className="grid grid-cols-[0fr] opacity-0 transition-[grid-template-columns,opacity] duration-base ease-out-quick group-aria-pressed/toggle:grid-cols-[1fr] group-aria-pressed/toggle:opacity-100 motion-safe:[transition-delay:90ms,0ms] motion-safe:group-aria-pressed/toggle:[transition-delay:0ms,110ms]">
              <span className="overflow-hidden">
                <span className="pl-1.5">{label}</span>
              </span>
            </span>
          </ToggleGroupItem>
        )
      })}
    </ToggleGroup>
  )
}
