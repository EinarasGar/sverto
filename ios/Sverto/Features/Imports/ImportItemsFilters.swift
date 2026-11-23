import Foundation
import SvertoCore

enum ItemsVerbFilter: CaseIterable, Hashable {
    case all
    case add
    case amend
    case skip
    case delete

    var label: String {
        switch self {
        case .all: "All"
        case .add: "Add"
        case .amend: "Amend"
        case .skip: "Skip"
        case .delete: "Delete"
        }
    }

    var verb: ImportRowVerb? {
        switch self {
        case .all: nil
        case .add: .add
        case .amend: .amend
        case .skip: .skip
        case .delete: .delete
        }
    }
}

struct ItemsAccountChoice: Identifiable, Hashable {
    let id: String
    let name: String
    let count: Int
}

enum ImportItemsFilters {
    static func needsALook(_ row: ImportStagedRow) -> Bool {
        row.verb == .amend
            || row.verb == .delete
            || row.kind == .joinedTransfer
            || !row.nearMiss.isEmpty
            || (row.typeKey == "regular" && (row.categoryName == nil || row.categoryName == "Miscellaneous"))
    }

    static func accountChoices(_ rows: [ImportStagedRow]) -> [ItemsAccountChoice] {
        var order: [String] = []
        var byAccount: [String: [ImportStagedRow]] = [:]
        for row in rows {
            if byAccount[row.accountId] == nil { order.append(row.accountId) }
            byAccount[row.accountId, default: []].append(row)
        }
        return order
            .compactMap { id in
                byAccount[id].flatMap { accountRows in
                    accountRows.first.map { first in
                        let name = first.accountName.components(separatedBy: " → ").first ?? first.accountName
                        return ItemsAccountChoice(id: id, name: name, count: accountRows.count)
                    }
                }
            }
            .sorted { $0.count > $1.count }
    }

    static func matches(_ row: ImportStagedRow, query: String) -> Bool {
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        if text.isEmpty { return true }
        if row.description.localizedCaseInsensitiveContains(text) { return true }
        let digits = text.filter { $0.isNumber || $0 == "." }
        guard digits.contains(where: \.isNumber) else { return false }
        return row.amountDisplay.filter { $0.isNumber || $0 == "." }.contains(digits)
    }

    static func groupedByDay(_ rows: [ImportStagedRow]) -> [(label: String, rows: [ImportStagedRow])] {
        var order: [String] = []
        var byDay: [String: [ImportStagedRow]] = [:]
        for row in rows.sorted(by: { $0.date > $1.date }) {
            let label = ImportDates.date(row.date)
            if byDay[label] == nil { order.append(label) }
            byDay[label, default: []].append(row)
        }
        return order.map { ($0, byDay[$0] ?? []) }
    }
}
