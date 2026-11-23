import Foundation

enum ConnectorNames {
    nonisolated static func humanizeAccountName(_ raw: String) -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count > 20, !trimmed.contains(" ") {
            return "Account · \(trimmed.suffix(4))"
        }
        return trimmed.lowercased()
            .split(separator: " ", omittingEmptySubsequences: false)
            .map { $0.prefix(1).uppercased() + $0.dropFirst() }
            .joined(separator: " ")
    }

    static func providerName(_ providerKind: String) -> String {
        ConnectorKind(rawValue: providerKind)?.info.name ?? providerKind
    }

    nonisolated static func quantity(_ value: Double) -> String {
        QuantityText.format(value, locale: .current)
    }
}
