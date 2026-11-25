import SvertoCore
import SwiftUI

struct AssetChartSection<CurrencyControl: View>: View {
    let showsSeriesSwitch: Bool
    @Binding var series: HoldingChartSeries
    @Binding var period: ChartPeriod
    let periods: [ChartPeriodData]
    let ticker: String
    let isLoading: Bool
    let error: String?
    let retry: () async -> Void
    @ViewBuilder let currencyControl: () -> CurrencyControl

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                if showsSeriesSwitch {
                    HoldingChartSwitch(selection: $series)
                }
                Spacer(minLength: 8)
                if series != .units {
                    currencyControl()
                }
            }
            chart
        }
    }

    @ViewBuilder
    private var chart: some View {
        if isLoading {
            ChartSkeleton()
        } else if let error {
            ContentUnavailableView {
                Label("Couldn't load chart", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") {
                    Task { await retry() }
                }
                .buttonStyle(.bordered)
            }
        } else {
            ChartView(
                periods: periods,
                baseTicker: ticker,
                selectedPeriod: $period,
                caption: showsSeriesSwitch ? series.caption : nil,
                valueFormat: series.valueFormat
            )
        }
    }
}
