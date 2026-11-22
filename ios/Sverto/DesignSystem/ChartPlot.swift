import Charts
import SvertoCore
import SwiftUI

struct ChartPlot: View, Equatable {
    let series: ChartSeries
    let selection: ChartSelection
    let isPositive: Bool

    @State private var revealProgress: CGFloat = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.series === rhs.series && lhs.selection === rhs.selection && lhs.isPositive == rhs.isPositive
    }

    private var lineColor: Color { isPositive ? .brand : .loss }

    var body: some View {
        Chart {
            AreaPlot(
                series.samples,
                x: .value("Date", \ChartSample.date),
                yStart: .value("Floor", series.domain.lowerBound),
                yEnd: .value("Value", \ChartSample.value)
            )
                .interpolationMethod(.monotone)
                .foregroundStyle(
                    LinearGradient(colors: [lineColor.opacity(0.35), lineColor.opacity(0)], startPoint: .top, endPoint: .bottom)
                )
                .accessibilityHidden(true)
            LinePlot(series.samples, x: .value("Date", \ChartSample.date), y: .value("Value in \(series.ticker)", \ChartSample.value))
                .accessibilityLabel(\ChartSample.accessibilityDate)
                .accessibilityValue(\ChartSample.formattedValue)
                .interpolationMethod(.monotone)
                .foregroundStyle(lineColor)
                .lineStyle(StrokeStyle(lineWidth: 2.5, lineCap: .round, lineJoin: .round))
            ForEach(series.markers, id: \.timestamp) { marker in
                PointMark(
                    x: .value("Date", DateText.date(fromUnix: marker.timestamp)),
                    y: .value("Valuation in \(series.ticker)", marker.value)
                )
                .foregroundStyle(lineColor)
                .symbolSize(60)
                .accessibilityLabel(Text(DateText.date(fromUnix: marker.timestamp), format: .dateTime.year().month().day()))
                .accessibilityValue(Money.format(marker.value, ticker: series.ticker))
            }
        }
        .chartYScale(domain: series.domain.range)
        .chartXScale(range: .plotDimension(padding: series.markers.isEmpty ? 0 : 6))
        .chartXAxis(.hidden)
        .chartYAxis(.hidden)
        .chartPlotStyle { plotArea in
            plotArea.mask(alignment: .leading) {
                Rectangle().scaleEffect(x: reduceMotion ? 1 : revealProgress, y: 1, anchor: .leading)
            }
        }
        .chartXSelection(value: Binding(
            get: { selection.date },
            set: {
                selection.date = $0
                selection.index = series.nearestIndex(to: $0)
                selection.marker = series.nearestMarker(to: $0)
            }
        ))
        .chartOverlay { proxy in
            ChartSelectionOverlay(series: series, selection: selection, proxy: proxy, lineColor: lineColor)
        }
        .opacity(series.samples.count < 2 ? 0 : 1)
        .accessibilityLabel("Value chart in \(series.ticker)")
        .task(id: series.samples.count >= 2) {
            guard series.samples.count >= 2, revealProgress == 0 else { return }
            guard !reduceMotion else {
                revealProgress = 1
                return
            }
            do {
                try await Task.sleep(for: .milliseconds(16))
            } catch {
                return
            }
            withAnimation(.easeOut(duration: 0.6)) { revealProgress = 1 }
        }
        .onChange(of: reduceMotion) { _, reduced in
            if reduced { revealProgress = 1 }
        }
    }
}

private struct ChartSelectionOverlay: View {
    let series: ChartSeries
    let selection: ChartSelection
    let proxy: ChartProxy
    let lineColor: Color

    var body: some View {
        GeometryReader { geometry in
            if let point = selection.marker ?? selection.index.flatMap({ series.points.indices.contains($0) ? series.points[$0] : nil }),
               let plotFrame = proxy.plotFrame,
               let x = proxy.position(forX: DateText.date(fromUnix: point.timestamp)),
               let y = proxy.position(forY: point.value) {
                let frame = geometry[plotFrame]
                Rectangle()
                    .fill(Color.secondary.opacity(0.3))
                    .frame(width: 1, height: frame.height)
                    .position(x: frame.minX + x, y: frame.midY)
                Circle()
                    .fill(.cardFill)
                    .frame(width: 12, height: 12)
                    .overlay {
                        Circle().fill(lineColor).frame(width: 8, height: 8)
                    }
                    .position(x: frame.minX + x, y: frame.minY + y)
            }
        }
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }
}
