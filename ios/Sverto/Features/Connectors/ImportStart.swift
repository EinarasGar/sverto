import Foundation
import SvertoCore

enum ImportStart: CaseIterable {
    case lastEntry
    case all
    case date

    var label: String {
        switch self {
        case .lastEntry: "My last entry"
        case .all: "All available"
        case .date: "A date"
        }
    }

    func importFrom(startDate: Date) -> ConnectorImportFrom {
        switch self {
        case .lastEntry: .afterLastEntry
        case .all: .all
        case .date: .date(date: Self.utcMidnight(of: startDate))
        }
    }

    private static func utcMidnight(of date: Date) -> Int64 {
        var utc = Calendar(identifier: .gregorian)
        utc.timeZone = .gmt
        let day = Calendar.current.dateComponents([.year, .month, .day], from: date)
        return Int64((utc.date(from: day) ?? date).timeIntervalSince1970)
    }
}
