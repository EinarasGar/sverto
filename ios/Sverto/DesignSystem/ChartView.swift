import SwiftUI
import SvertoCore

struct ChartView: View {
    private let periods: [ChartPeriodData]
    private let baseTicker: String
    @Binding private var selectedPeriod: ChartPeriod
    private let showHeadline: Bool
    private let showsPeriodPicker: Bool
    private let changeSuffix: String?
    private let markers: [ChartPoint]
    private let plotOutset: CGFloat
    private let caption: String?
    private let valueFormat: ChartValueFormat

    @State private var selection = ChartSelection()
    @State private var seriesCache = ChartSeriesCache()
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.horizontalSizeClass) private var sizeClass

    init(
        periods: [ChartPeriodData],
        baseTicker: String,
        selectedPeriod: Binding<ChartPeriod>,
        showHeadline: Bool = true,
        showsPeriodPicker: Bool = true,
        changeSuffix: String? = nil,
        markers: [ChartPoint] = [],
        plotOutset: CGFloat = 0,
        caption: String? = nil,
        valueFormat: ChartValueFormat = .money(showsChangePercent: true)
    ) {
        self.periods = periods
        self.baseTicker = baseTicker
        _selectedPeriod = selectedPeriod
        self.showHeadline = showHeadline
        self.showsPeriodPicker = showsPeriodPicker
        self.changeSuffix = changeSuffix
        self.markers = markers
        self.plotOutset = plotOutset
        self.caption = caption
        self.valueFormat = valueFormat
    }

    private var series: ChartSeries {
        seriesCache.series(
            points: periods.first { $0.period == selectedPeriod.rawValue }?.points ?? [],
            ticker: baseTicker,
            markers: markers
        )
    }

    private var availablePeriods: [ChartPeriod] {
        let available = periods.compactMap { ChartPeriod(rawValue: $0.period) }
        return available.isEmpty ? ChartPeriod.allCases : available
    }

    private func formatValue(_ value: Double) -> String {
        switch valueFormat {
        case .money: Money.format(value, ticker: baseTicker)
        case .units: QuantityText.format(value, locale: DateText.androidParityLocale)
        }
    }

    private func formatChange(_ change: Double) -> String {
        switch valueFormat {
        case .money: Money.format(change, ticker: baseTicker, signed: true)
        case .units: (change > 0 ? "+" : "") + QuantityText.format(change, locale: DateText.androidParityLocale)
        }
    }

    private var changeText: String {
        var text = formatChange(summary.change)
        if valueFormat == .money(showsChangePercent: true) {
            text += " (\(PercentText.format(summary.changeFraction, fractionDigits: 2)))"
        }
        if let changeSuffix {
            text += " \(changeSuffix)"
        }
        return text
    }

    private var summary: ChartPointSummary {
        ChartPointSummary(points: series.points, scrubbedIndex: selection.index, scrubbedMarker: selection.marker)
    }

    var body: some View {
        let series = self.series
        VStack(alignment: .leading, spacing: 16) {
            if showHeadline {
                headline
                    .padding(.horizontal, -plotOutset)
            }
            ChartPlot(series: series, selection: selection, isPositive: summary.isPositive)
                .equatable()
                .id(selectedPeriod)
                .frame(height: sizeClass == .regular ? 240 : 180)
                .padding(.horizontal, -plotOutset)
                .accessibilityValue(formatValue(summary.currentValue))
            if showsPeriodPicker {
                periodPicker
                    .padding(.horizontal, -plotOutset)
            }
        }
        .sensoryFeedback(.selection, trigger: selection.index) { _, index in index != nil }
        .sensoryFeedback(.selection, trigger: selectedPeriod)
        .onChange(of: selectedPeriod) {
            selection.clear()
        }
        .onChange(of: series.points) {
            selection.clear()
        }
    }

    private var headline: some View {
        VStack(alignment: .leading, spacing: 4) {
            if let caption {
                Text(caption)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            Text(formatValue(summary.currentValue))
                .font(.largeTitle.weight(.semibold))
                .monospacedDigit()
                .contentTransition(reduceMotion ? .identity : .numericText())
                .animation(reduceMotion ? nil : .interactiveSpring(response: 0.16, dampingFraction: 0.9), value: summary.currentValue)
            Text(changeText)
                .font(.subheadline)
                .monospacedDigit()
                .foregroundStyle(summary.isPositive ? Color.gain : Color.loss)
            Text(scrubDateText ?? " ")
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .opacity(scrubDateText == nil ? 0 : 1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }

    private var scrubDateText: String? {
        if let marker = selection.marker {
            return "Valued \(formatValue(marker.value)) · \(DateText.chartScrubLabel(epochSeconds: marker.timestamp, intraday: false))"
        }
        return summary.scrubbedTimestamp.map { DateText.chartScrubLabel(epochSeconds: $0, intraday: selectedPeriod.isIntraday) }
    }

    @ViewBuilder
    private var periodPicker: some View {
        if dynamicTypeSize.isAccessibilitySize {
            HStack {
                Text("Period")
                    .foregroundStyle(Color.secondary)
                Spacer()
                periodOptions
                    .pickerStyle(.menu)
            }
        } else {
            periodOptions
                .pickerStyle(.segmented)
                .labelsHidden()
        }
    }

    private var periodOptions: some View {
        Picker("Period", selection: $selectedPeriod) {
            ForEach(availablePeriods, id: \.self) { period in
                Text(period.rawValue).tag(period)
            }
        }
    }
}

struct ChartSkeleton: View {
    var showHeadline = true
    var showsPeriodPicker = true
    var plotOutset: CGFloat = 0
    @Environment(\.horizontalSizeClass) private var sizeClass

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if showHeadline {
                VStack(alignment: .leading, spacing: 8) {
                    SkeletonBlock(width: 180, height: 32)
                    SkeletonBlock(width: 140, height: 16)
                }
                .padding(.horizontal, -plotOutset)
            }
            SkeletonBlock(height: sizeClass == .regular ? 240 : 180)
                .padding(.horizontal, -plotOutset)
            if showsPeriodPicker {
                HStack(spacing: 8) {
                    ForEach(ChartPeriod.allCases, id: \.self) { _ in
                        SkeletonBlock(height: 32)
                    }
                }
                .padding(.horizontal, -plotOutset)
            }
        }
        .accessibilityLabel("Loading chart")
    }
}

private func previewPeriods() -> [ChartPeriodData] {
    let now = Int64(Date().timeIntervalSince1970)
    var points: [ChartPoint] = []
    for day in 0..<30 {
        let timestamp = now - Int64((29 - day) * 86_400)
        let value = 40_000 + Double(day) * 120 + Double((day * 37) % 900)
        points.append(ChartPoint(timestamp: timestamp, value: value))
    }
    return ChartPeriod.allCases.map { ChartPeriodData(period: $0.rawValue, points: points) }
}

#Preview {
    @Previewable @State var period = ChartPeriod.defaultSelection
    VStack(spacing: 32) {
        ChartView(periods: previewPeriods(), baseTicker: "USD", selectedPeriod: $period)
        ChartSkeleton()
    }
    .padding()
    .background(.screenBackground)
}
