import Foundation
import SvertoCore

struct AccountAsk: Hashable {
    let askKey: String
    let sourceId: String
    let rawAccount: String
    let candidates: [ImportAccountCandidate]
    let canExclude: Bool
    let answerCoversAllSources: Bool
}

struct AssetAsk: Hashable {
    let askKey: String
    let raw: String
    let candidates: [ImportAssetCandidate]
    let canExclude: Bool
}

struct BalanceAsk: Hashable {
    let askKey: String
    let accountId: String
    let accountName: String
    let assetId: Int32
    let ticker: String
    let asOfDate: Int64
}

struct BankAccountSource: Hashable {
    let providerKind: String
    let displayName: String?
    let currency: String?
    let accountNumberLast4: String?

    var label: String {
        if let displayName, !displayName.trimmingCharacters(in: .whitespaces).isEmpty {
            return ConnectorNames.humanizeAccountName(displayName)
        }
        return providerName
    }

    var providerName: String {
        ConnectorNames.providerName(providerKind)
    }
}

extension ImportSource {
    var file: ImportFile? {
        if case let .file(file) = kind { file } else { nil }
    }

    var bankAccount: BankAccountSource? {
        guard case let .bankAccount(_, providerKind, displayName, currency, last4) = kind else { return nil }
        return BankAccountSource(providerKind: providerKind, displayName: displayName, currency: currency, accountNumberLast4: last4)
    }

    var isBankAccount: Bool { bankAccount != nil }

    var label: String {
        file?.name ?? bankAccount?.label ?? ""
    }
}

extension ImportSessionDetail {
    var files: [ImportFile] { sources.compactMap(\.file) }

    var accountAsks: [AccountAsk] {
        asks.compactMap { ask in
            guard case let .accountMapping(askKey, sourceId, rawAccount, candidates, canExclude, coversAll) = ask else { return nil }
            return AccountAsk(
                askKey: askKey,
                sourceId: sourceId,
                rawAccount: rawAccount,
                candidates: candidates,
                canExclude: canExclude,
                answerCoversAllSources: coversAll
            )
        }
    }

    var assetAsks: [AssetAsk] {
        asks.compactMap { ask in
            guard case let .assetMapping(askKey, raw, candidates, canExclude) = ask else { return nil }
            return AssetAsk(askKey: askKey, raw: raw, candidates: candidates, canExclude: canExclude)
        }
    }

    var balanceAsks: [BalanceAsk] {
        asks.compactMap { ask in
            guard case let .noCheckpoint(askKey, accountId, accountName, assetId, ticker, asOfDate) = ask else { return nil }
            return BalanceAsk(
                askKey: askKey,
                accountId: accountId,
                accountName: accountName,
                assetId: assetId,
                ticker: ticker,
                asOfDate: asOfDate
            )
        }
    }
}

struct AccountQuestion: Hashable {
    let rawAccount: String
    let asks: [AccountAsk]

    var coversAll: Bool { asks.contains(where: \.answerCoversAllSources) }
    var splittable: Bool { asks.count > 1 && !coversAll }
    var candidates: [ImportAccountCandidate] {
        var seen = Set<String>()
        return asks.flatMap(\.candidates).filter { seen.insert($0.id).inserted }
    }

    func targets(separate: Bool) -> [AccountAsk] {
        coversAll || separate ? Array(asks.prefix(1)) : asks
    }
}

enum SessionPhase {
    case working
    case needsInput
    case ready
    case completed
}

struct AppliedCounts: Equatable {
    let added: Int
    let corrected: Int
    let deleted: Int
    let skipped: Int
}

struct StatusHeaderState: Equatable {
    let label: String
    let summary: String
    let tone: ImportTone
    let systemImage: String
    let working: Bool
    var error: String? = nil
}

struct BalanceLine: Equatable {
    let text: String
    let ok: Bool
}

struct OutcomeState: Equatable {
    let headline: String
    let breakdown: String?
    let scope: String
    let balance: BalanceLine?
}

enum DecisionTarget: Hashable {
    case block(String)
    case row(String)
}

struct DecisionNavItem: Hashable {
    let label: String
    let progress: String?
    let target: DecisionTarget
}

struct BalanceCheckBlock: Equatable {
    let check: ImportBalanceCheck
    let question: BalanceAsk?
    let entered: ImportEnteredBalance?
    let adjustments: [ImportStagedRow]
}

enum SessionBlock: Identifiable, Equatable {
    case hero(StatusHeaderState, retryCategorizing: Bool)
    case receipt([String])
    case questionsHeading(Int)
    case section(title: String, count: String? = nil, addAllIds: [String] = [])
    case decisionNav([DecisionNavItem])
    case outcome(OutcomeState)
    case rows([ImportStagedRow], total: Int)
    case accountQuestion(AccountQuestion)
    case assetQuestion(AssetAsk)
    case balanceQuestion(BalanceAsk)
    case files(collapsible: Bool)
    case pending(count: Int64)
    case duplicate(ImportStagedRow, progress: String?)
    case startingBalance(ImportStagedRow)
    case balancedAccounts([ImportBalanceCheck])
    case balanceCheck(BalanceCheckBlock)
    case note(String)
    case unstageable(count: Int)

    static func duplicateId(_ rowId: String) -> String { "duplicate-\(rowId)" }
    static func startId(_ rowId: String) -> String { "start-\(rowId)" }

    var id: String {
        switch self {
        case .hero: "hero"
        case .receipt: "receipt"
        case .questionsHeading: "questions"
        case let .section(title, _, _): "section-\(title)"
        case .decisionNav: "decisions"
        case .outcome: "outcome"
        case .rows: "rows"
        case let .accountQuestion(question): "account-\(question.rawAccount)"
        case let .assetQuestion(ask): "asset-\(ask.askKey)"
        case let .balanceQuestion(ask): "balance-ask-\(ask.askKey)"
        case .files: "files"
        case .pending: "pending"
        case let .duplicate(row, _): Self.duplicateId(row.id)
        case let .startingBalance(row): Self.startId(row.id)
        case .balancedAccounts: "balanced"
        case let .balanceCheck(block): "check-\(block.check.accountId)-\(block.check.assetId)"
        case let .note(text): "note-\(text)"
        case .unstageable: "unstageable"
        }
    }
}

struct ApplyConfirm: Equatable {
    let summary: String
    let warnings: [String]
    let risky: Bool
}

enum ApplyBarState: Equatable {
    case apply(label: String, summary: String?, confirm: ApplyConfirm?)
    case navigate(label: String, target: DecisionTarget)
    case disabled(text: String)
}

struct ImportToast: Equatable {
    let id = UUID()
    let text: String
}
