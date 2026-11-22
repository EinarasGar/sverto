import Foundation
import SvertoCore

struct ChartPointSummary: Equatable {
    let currentValue: Double
    let change: Double
    let changeFraction: Double
    let scrubbedTimestamp: Int64?

    var isPositive: Bool { change >= 0 }

    nonisolated init(points: [ChartPoint], scrubbedIndex: Int?, scrubbedMarker: ChartPoint? = nil) {
        let scrubbed = scrubbedMarker ?? scrubbedIndex.flatMap { points.indices.contains($0) ? points[$0] : nil }
        let current = scrubbed?.value ?? points.last?.value ?? 0
        let start = points.first?.value ?? 0
        currentValue = current
        change = current - start
        changeFraction = start == 0 ? 0 : change / start
        scrubbedTimestamp = scrubbed?.timestamp
    }
}

struct ChartYDomain {
    let lowerBound: Double
    let upperBound: Double

    var range: ClosedRange<Double> { lowerBound...upperBound }

    nonisolated init(values: [Double]) {
        let maxValue = values.max() ?? 0
        let minValue = values.min() ?? 0
        let mid = (maxValue + minValue) / 2
        let span = max(maxValue - minValue, abs(mid) * 0.02, 1)
        let padding = span * 0.05
        lowerBound = mid - span / 2 - padding
        upperBound = mid + span / 2 + padding
    }
}
