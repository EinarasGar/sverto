import Foundation
import SvertoCore

struct SessionStatusCopy {
    let label: String
    let tone: ImportTone
    let summary: String
}

extension ImportSessionStatus {
    var copy: SessionStatusCopy {
        switch self {
        case .collecting:
            SessionStatusCopy(
                label: "Waiting for files",
                tone: .ghost,
                summary: "Add statements, exports, spreadsheets or screenshots. Sverto works out which accounts they belong to."
            )
        case .parsing:
            SessionStatusCopy(
                label: "Reading files",
                tone: .brand,
                summary: "Sverto is reading the files. It keeps going in the background, so it is safe to leave this page."
            )
        case .needsInput:
            SessionStatusCopy(
                label: "Needs your input",
                tone: .attention,
                summary: "Something needs a decision from you before the import can continue."
            )
        case .reconciling:
            SessionStatusCopy(
                label: "Reconciling",
                tone: .brand,
                summary: "Comparing the imported transactions with your ledger."
            )
        case .proposalReady:
            SessionStatusCopy(
                label: "Ready to review",
                tone: .positive,
                summary: "The proposed ledger changes are ready. Nothing is written until you approve them."
            )
        case .completed:
            SessionStatusCopy(
                label: "Completed",
                tone: .positive,
                summary: "The approved changes are in your ledger."
            )
        }
    }

    func applyBlocker(failedFiles: Bool) -> String? {
        switch self {
        case .collecting: "Add files first."
        case .parsing: "Wait until every file has been read."
        case .needsInput:
            failedFiles
                ? "Answer the open questions and retry or remove the files that couldn't be read."
                : "Answer the open questions first."
        case .reconciling: "Wait until the transactions are categorized."
        case .proposalReady, .completed: nil
        }
    }
}

extension ImportRowKind {
    var note: String? {
        switch self {
        case .movement: nil
        case .startingBalance:
            "Starting balance: added before the statement's first row so that its first balance matches your ledger."
        case .joinedTransfer:
            "Moved between your accounts: the row going out and the row coming in, imported as one transfer."
        case .adjustment:
            "Adjustment: closes a difference between two statement balances that nothing in your ledger explains. Left out unless you add it."
        }
    }
}

extension ImportAmendRefusal {
    var note: String {
        switch self {
        case .targetGone:
            "This correction can't be made: the ledger transaction it matched is gone. Undo the pin to decide again."
        case .notOneEntry:
            "This correction can't be made: the ledger transaction doesn't have exactly one entry on this account and asset. Undo the pin to decide again."
        case .unreadable:
            "This correction can't be made: the ledger transaction can't be read. Undo the pin to decide again."
        }
    }
}

extension ImportRowVerb {
    var label: String {
        switch self {
        case .add: "Add"
        case .amend: "Amend"
        case .delete: "Delete"
        case .skip: "Skip"
        }
    }

    var tone: ImportTone {
        switch self {
        case .add: .positive
        case .amend: .brand
        case .delete: .negative
        case .skip: .ghost
        }
    }
}

enum ImportCopy {
    static func formatCount<Count: BinaryInteger>(_ count: Count) -> String {
        Int(count).formatted()
    }

    static func countOf<Count: BinaryInteger>(_ count: Count, _ noun: String, plural: String? = nil) -> String {
        count == 1 ? "1 \(noun)" : "\(formatCount(count)) \(plural ?? "\(noun)s")"
    }

    static func sourcesCount(_ sources: [ImportSource], uploads: Int = 0) -> String {
        let bankAccounts = sources.filter(\.isBankAccount).count
        let files = sources.count - bankAccounts + uploads
        return [
            files > 0 ? countOf(files, "file") : nil,
            bankAccounts > 0 ? countOf(bankAccounts, "bank account") : nil,
        ]
        .compactMap { $0 }
        .joined(separator: ", ")
    }
}
