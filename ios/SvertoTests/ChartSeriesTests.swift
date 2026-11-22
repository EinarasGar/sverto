import Foundation
import SvertoCore
import Testing
@testable import Sverto

private func dailyPoints(_ values: [Double]) -> [ChartPoint] {
    values.enumerated().map { ChartPoint(timestamp: Int64($0.offset) * 86_400, value: $0.element) }
}

struct ChartSeriesTests {
    @Test func scrubbingNearAValuationSnapsToIt() {
        let valuation = ChartPoint(timestamp: 50 * 86_400, value: 500)
        let series = ChartSeries(points: dailyPoints(Array(repeating: 100, count: 101)), ticker: "GBP", markers: [valuation])

        #expect(series.nearestMarker(to: Date(timeIntervalSince1970: 52 * 86_400)) == valuation)
        #expect(series.nearestMarker(to: Date(timeIntervalSince1970: 60 * 86_400)) == nil)
        #expect(series.domain.range.contains(500))
    }

    @Test func aScrubbedValuationIsWhatTheHeadlineShows() {
        let points = dailyPoints([100, 110, 120])
        let summary = ChartPointSummary(points: points, scrubbedIndex: 1, scrubbedMarker: ChartPoint(timestamp: 86_400, value: 150))

        #expect(summary.currentValue == 150)
        #expect(summary.change == 50)
        #expect(summary.scrubbedTimestamp == 86_400)
    }

    @Test func shortSeriesIsPlottedInFull() {
        let points = dailyPoints([3, 1, 2])
        #expect(ChartSeries.plottedIndices(of: points) == [0, 1, 2])
    }

    @Test func longSeriesIsCappedAndKeepsEndsAndExtremes() {
        var values = (0..<11_551).map { Double($0 % 97) }
        values[4_000] = 1_000
        values[7_000] = -1_000
        let points = dailyPoints(values)

        let plotted = ChartSeries.plottedIndices(of: points)

        #expect(plotted.count <= ChartSeries.maxPlottedSamples)
        #expect(plotted.first == 0)
        #expect(plotted.last == 11_550)
        #expect(plotted.contains(4_000))
        #expect(plotted.contains(7_000))
        #expect(plotted == plotted.sorted())
    }

    @Test func unorderedPointsArePlottedChronologically() {
        let points = [
            ChartPoint(timestamp: 300, value: 3),
            ChartPoint(timestamp: 100, value: 1),
            ChartPoint(timestamp: 200, value: 2),
        ]
        #expect(ChartSeries.plottedIndices(of: points) == [1, 2, 0])
    }

    @Test func scrubbingSnapsToAPlottedPointAndReportsItsOriginalIndex() {
        let points = dailyPoints((0..<5_000).map(Double.init))
        let series = ChartSeries(points: points, ticker: "USD")
        let plottedIds = Set(series.samples.map(\.id))

        let index = series.nearestIndex(to: DateText.date(fromUnix: 2_500 * 86_400))

        #expect(index.map(plottedIds.contains) == true)
        #expect(series.samples.count <= ChartSeries.maxPlottedSamples)
    }
}
