import Foundation
import Observation
import SwiftUI
import SvertoCore

struct ChartSample {
    let id: Int
    let date: Date
    let value: Double
    let formattedValue: String
    let accessibilityDate: Text
}

final class ChartSeriesCache {
    private var current: ChartSeries?

    func series(points: [ChartPoint], ticker: String, markers: [ChartPoint] = []) -> ChartSeries {
        if let current, current.ticker == ticker, current.points == points, current.markers == markers {
            return current
        }
        let series = ChartSeries(points: points, ticker: ticker, markers: markers)
        current = series
        return series
    }
}

final class ChartSeries {
    let points: [ChartPoint]
    let markers: [ChartPoint]
    let samples: [ChartSample]
    let domain: ChartYDomain
    let ticker: String
    private let chronologicalSamples: [ChartSample]

    nonisolated static let maxPlottedSamples = 600
    nonisolated static let markerReachFraction = 0.03

    init(points: [ChartPoint], ticker: String, markers: [ChartPoint] = []) {
        self.points = points
        self.markers = markers.sorted { $0.timestamp < $1.timestamp }
        self.ticker = ticker
        samples = Self.plottedIndices(of: points).map { index in
            let point = points[index]
            let date = DateText.date(fromUnix: point.timestamp)
            return ChartSample(
                id: index,
                date: date,
                value: point.value,
                formattedValue: Money.format(point.value, ticker: ticker),
                accessibilityDate: Text(date, format: .dateTime.year().month().day().hour().minute())
            )
        }
        chronologicalSamples = samples
        domain = ChartYDomain(values: points.map(\.value) + markers.map(\.value))
    }

    // Swift Charts re-renders every mark on each scrub update, so plotting thousands of points
    // (e.g. a decade of daily rates) drops scrubbing to a few frames per second. Keeping each
    // bucket's low and high preserves the visible shape at a fraction of the marks.
    nonisolated static func plottedIndices(of points: [ChartPoint]) -> [Int] {
        let chronological = points.indices.sorted { points[$0].timestamp < points[$1].timestamp }
        guard chronological.count > maxPlottedSamples, let first = chronological.first, let last = chronological.last else {
            return chronological
        }
        let interior = chronological[1..<(chronological.count - 1)]
        let bucketCount = (maxPlottedSamples - 2) / 2
        let bucketSize = Double(interior.count) / Double(bucketCount)
        var plotted = [first]
        plotted.reserveCapacity(maxPlottedSamples)
        for bucket in 0..<bucketCount {
            let start = interior.startIndex + Int(Double(bucket) * bucketSize)
            let end = interior.startIndex + Int(Double(bucket + 1) * bucketSize)
            let slice = interior[start..<min(end, interior.endIndex)]
            guard let low = slice.min(by: { points[$0].value < points[$1].value }),
                  let high = slice.max(by: { points[$0].value < points[$1].value }) else { continue }
            if low == high {
                plotted.append(low)
            } else if points[low].timestamp <= points[high].timestamp {
                plotted.append(contentsOf: [low, high])
            } else {
                plotted.append(contentsOf: [high, low])
            }
        }
        plotted.append(last)
        return plotted
    }

    func nearestMarker(to date: Date?) -> ChartPoint? {
        let timestamps = points.map(\.timestamp)
        guard let date, let first = timestamps.min(), let last = timestamps.max(), last > first else { return nil }
        let reach = Double(last - first) * Self.markerReachFraction
        let seconds = date.timeIntervalSince1970
        let distance = { (marker: ChartPoint) in abs(Double(marker.timestamp) - seconds) }
        return markers.filter { distance($0) <= reach }.min { distance($0) < distance($1) }
    }

    func nearestIndex(to date: Date?) -> Int? {
        guard let date, chronologicalSamples.count >= 2 else { return nil }
        var lower = 0
        var upper = chronologicalSamples.count
        while lower < upper {
            let middle = (lower + upper) / 2
            if chronologicalSamples[middle].date < date {
                lower = middle + 1
            } else {
                upper = middle
            }
        }
        guard lower > 0 else { return chronologicalSamples.first?.id }
        guard lower < chronologicalSamples.count else { return chronologicalSamples.last?.id }
        let previous = chronologicalSamples[lower - 1]
        let next = chronologicalSamples[lower]
        return date.timeIntervalSince(previous.date) <= next.date.timeIntervalSince(date) ? previous.id : next.id
    }
}

@Observable
final class ChartSelection {
    var date: Date?
    var index: Int?
    var marker: ChartPoint?

    func clear() {
        date = nil
        index = nil
        marker = nil
    }
}
