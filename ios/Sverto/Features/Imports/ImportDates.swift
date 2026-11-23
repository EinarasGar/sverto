import Foundation

enum ImportDates {
    private static let utc = TimeZone(identifier: "UTC") ?? .gmt
    private static var calendar: Calendar {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = utc
        return calendar
    }

    private static var longStyle: Date.VerbatimFormatStyle {
        Date.VerbatimFormatStyle(
            format: "\(day: .defaultDigits) \(month: .abbreviated) \(year: .defaultDigits)",
            locale: .current,
            timeZone: utc,
            calendar: calendar
        )
    }

    private static var shortStyle: Date.VerbatimFormatStyle {
        Date.VerbatimFormatStyle(
            format: "\(day: .defaultDigits) \(month: .abbreviated)",
            locale: .current,
            timeZone: utc,
            calendar: calendar
        )
    }

    static func date(_ epochSeconds: Int64) -> String {
        DateText.date(fromUnix: epochSeconds).formatted(longStyle)
    }

    static func shortDate(_ epochSeconds: Int64) -> String {
        DateText.date(fromUnix: epochSeconds).formatted(shortStyle)
    }

    static func range(_ first: Int64, _ last: Int64) -> String {
        let from = DateText.date(fromUnix: first)
        let to = DateText.date(fromUnix: last)
        if calendar.isDate(from, inSameDayAs: to) {
            return date(first)
        }
        if calendar.component(.year, from: from) == calendar.component(.year, from: to) {
            return "\(shortDate(first)) – \(date(last))"
        }
        return "\(date(first)) – \(date(last))"
    }

    static func time(_ epochSeconds: Int64) -> String {
        DateText.date(fromUnix: epochSeconds).formatted(.dateTime.day().month(.abbreviated).hour().minute())
    }
}
