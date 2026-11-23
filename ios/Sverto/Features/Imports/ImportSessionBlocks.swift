import Foundation
import SvertoCore

enum ImportSessionBlocks {
    private static let inlineRows = 8
    private static let previewRowCount = 5

    static func phase(detail: ImportSessionDetail, items: ImportItems?) -> SessionPhase {
        let ready = items?.ready == true
        let questions = !detail.accountAsks.isEmpty || !detail.assetAsks.isEmpty || (!ready && !detail.balanceAsks.isEmpty)
        if detail.summary.status == .completed { return .completed }
        if questions { return .needsInput }
        if detail.summary.status == .proposalReady, ready { return .ready }
        return .working
    }

    static func subtitle(_ detail: ImportSessionDetail) -> String {
        let label = detail.summary.status.copy.label
        return detail.sources.isEmpty ? label : "\(label) · \(ImportCopy.sourcesCount(detail.sources))"
    }

    static func blocks(
        detail: ImportSessionDetail,
        items: ImportItems?,
        seenDuplicates: Set<String>,
        applied: AppliedCounts?
    ) -> [SessionBlock] {
        let ready = items?.ready == true ? items : nil
        let anyFailed = detail.files.contains { $0.status == .failed }
        switch phase(detail: detail, items: items) {
        case .working:
            return [hero(detail), .files(collapsible: false)]
        case .needsInput:
            return questionBlocks(detail, items: ready) + [.files(collapsible: !anyFailed)]
        case .ready:
            guard let ready else { return [] }
            return readyBlocks(detail, items: ready, seenDuplicates: seenDuplicates) + [.files(collapsible: !anyFailed)]
        case .completed:
            return [.receipt(receiptLines(detail, applied: applied)), .files(collapsible: true)]
        }
    }

    static func applyBar(detail: ImportSessionDetail, items: ImportItems) -> ApplyBarState {
        if let blocker = detail.summary.status.applyBlocker(failedFiles: detail.files.contains { $0.status == .failed }) {
            return .disabled(text: blocker)
        }
        let duplicates = items.rows.filter { !$0.nearMiss.isEmpty }
        if let first = duplicates.first {
            return .navigate(
                label: "Review \(ImportCopy.countOf(duplicates.count, "possible duplicate"))",
                target: .block(SessionBlock.duplicateId(first.id))
            )
        }
        let starts = items.rows.filter(isUndecidedStart)
        if let first = starts.first {
            return .navigate(
                label: "Decide \(ImportCopy.countOf(starts.count, "starting balance"))",
                target: .block(SessionBlock.startId(first.id))
            )
        }
        let refusals = items.rows.filter { $0.amendRefusal != nil }
        if let first = refusals.first {
            return .navigate(label: "Fix \(ImportCopy.countOf(refusals.count, "correction"))", target: .row(first.id))
        }
        let add = verbCount(items, .add)
        let amend = verbCount(items, .amend)
        let delete = verbCount(items, .delete)
        let skip = verbCount(items, .skip)
        let addsOnly = amend == 0 && delete == 0
        let label = if addsOnly && add > 0 {
            "Add \(ImportCopy.countOf(add, "transaction"))"
        } else if addsOnly {
            "Finish import"
        } else {
            "Apply \(ImportCopy.countOf(add + amend + delete, "change"))"
        }
        let off = offBalanceCount(items)
        let warnings = [
            off > 0 ? "\(ImportCopy.countOf(off, "balance")) won't match the statements." : nil,
            delete > 0 ? "\(ImportCopy.countOf(delete, "transaction")) in your ledger will be deleted." : nil,
        ].compactMap { $0 }
        let parts = changeParts(add: add, amend: amend, delete: delete, skip: skip)
        let confirm = addsOnly && off == 0
            ? nil
            : ApplyConfirm(summary: parts.isEmpty ? "Nothing new to write" : parts, warnings: warnings, risky: !warnings.isEmpty)
        return .apply(
            label: label,
            summary: addsOnly ? nil : changeParts(add: add, amend: amend, delete: delete, skip: 0),
            confirm: confirm
        )
    }

    static func appliedCounts(_ items: ImportItems) -> AppliedCounts {
        AppliedCounts(
            added: verbCount(items, .add),
            corrected: verbCount(items, .amend),
            deleted: verbCount(items, .delete),
            skipped: verbCount(items, .skip)
        )
    }

    static func isUndecidedStart(_ row: ImportStagedRow) -> Bool {
        row.kind == .startingBalance && !row.verbLocked
    }

    static func isBalanced(_ check: ImportBalanceCheck) -> Bool {
        check.startingOffBy == 0 && check.finalOffBy == 0 && check.changes.isEmpty && !check.unlisted.contains { $0.askKey != nil }
    }

    static func offBalanceCount(_ items: ImportItems) -> Int {
        items.balanceChecks?.checks.filter { $0.startingOffBy != 0 || $0.finalOffBy != 0 || !$0.changes.isEmpty }.count ?? 0
    }

    private static func questionBlocks(_ detail: ImportSessionDetail, items: ImportItems?) -> [SessionBlock] {
        let accountQuestions = accountQuestions(detail.accountAsks)
        let assetAsks = detail.assetAsks
        let balanceAsks = items == nil ? detail.balanceAsks : []
        var blocks: [SessionBlock] = [.questionsHeading(accountQuestions.count + assetAsks.count + balanceAsks.count)]
        blocks += accountQuestions.map { .accountQuestion($0) }
        blocks += assetAsks.map { .assetQuestion($0) }
        blocks += balanceAsks.map { .balanceQuestion($0) }
        if detail.counts.pending > 0 {
            blocks.append(.pending(count: detail.counts.pending))
        }
        return blocks
    }

    private static func accountQuestions(_ asks: [AccountAsk]) -> [AccountQuestion] {
        var order: [String] = []
        var byRaw: [String: [AccountAsk]] = [:]
        for ask in asks {
            if byRaw[ask.rawAccount] == nil { order.append(ask.rawAccount) }
            byRaw[ask.rawAccount, default: []].append(ask)
        }
        return order.map { AccountQuestion(rawAccount: $0, asks: byRaw[$0] ?? []) }
    }

    private static func readyBlocks(_ detail: ImportSessionDetail, items: ImportItems, seenDuplicates: Set<String>) -> [SessionBlock] {
        var blocks: [SessionBlock] = []
        let duplicates = items.rows.filter { !$0.nearMiss.isEmpty }
        let total = seenDuplicates.union(duplicates.map(\.id)).count
        let resolved = total - duplicates.count
        let nav = decisionNav(items, duplicates: duplicates, resolved: resolved, total: total)
        if !nav.isEmpty {
            blocks.append(.decisionNav(nav))
        }
        blocks.append(.outcome(outcome(detail, items: items)))
        if !items.rows.isEmpty {
            blocks.append(.rows(previewRows(items.rows), total: items.rows.count))
        }
        if !duplicates.isEmpty {
            blocks.append(.section(title: "Possible duplicates", count: ImportCopy.formatCount(duplicates.count)))
            for (index, row) in duplicates.enumerated() {
                blocks.append(.duplicate(row, progress: total > 1 ? "\(resolved + index + 1) of \(total)" : nil))
            }
        }
        let starts = items.rows.filter(isUndecidedStart)
        if !starts.isEmpty {
            blocks.append(.section(title: "Starting balances", addAllIds: starts.count > 1 ? starts.map(\.id) : []))
            blocks += starts.map { .startingBalance($0) }
        }
        blocks += balanceBlocks(detail, items: items)
        if !items.unstageable.isEmpty {
            blocks.append(.unstageable(count: items.unstageable.count))
        }
        return blocks
    }

    private static func decisionNav(
        _ items: ImportItems,
        duplicates: [ImportStagedRow],
        resolved: Int,
        total: Int
    ) -> [DecisionNavItem] {
        var nav: [DecisionNavItem] = []
        if let first = duplicates.first {
            nav.append(DecisionNavItem(
                label: ImportCopy.countOf(duplicates.count, "possible duplicate"),
                progress: total > 1 ? "\(resolved + 1)/\(total)" : nil,
                target: .block(SessionBlock.duplicateId(first.id))
            ))
        }
        let starts = items.rows.filter(isUndecidedStart)
        if let first = starts.first {
            nav.append(DecisionNavItem(
                label: ImportCopy.countOf(starts.count, "starting balance"),
                progress: nil,
                target: .block(SessionBlock.startId(first.id))
            ))
        }
        let refusals = items.rows.filter { $0.amendRefusal != nil }
        if let first = refusals.first {
            nav.append(DecisionNavItem(
                label: ImportCopy.countOf(refusals.count, "correction to fix", plural: "corrections to fix"),
                progress: nil,
                target: .row(first.id)
            ))
        }
        return nav
    }

    private static func balanceBlocks(_ detail: ImportSessionDetail, items: ImportItems) -> [SessionBlock] {
        let balanceAsks = detail.balanceAsks
        guard let checks = items.balanceChecks,
              !checks.checks.isEmpty || checks.unchecked > 0 || !balanceAsks.isEmpty else { return [] }
        var blocks: [SessionBlock] = [.section(title: "Balances")]
        let adjustments = items.rows.filter { $0.kind == .adjustment }
        var asked = Set<String>()
        let detailed = checks.checks.map { check -> BalanceCheckBlock in
            let question = balanceAsks.first { $0.accountId == check.accountId && $0.assetId == check.assetId }
            if let question { asked.insert(question.askKey) }
            return BalanceCheckBlock(
                check: check,
                question: question,
                entered: checks.entered.first { $0.accountId == check.accountId && $0.assetId == check.assetId },
                adjustments: adjustments.filter { $0.accountId == check.accountId && $0.assetId == check.assetId }
            )
        }
        let isQuiet = { (block: BalanceCheckBlock) in
            block.question == nil
                && block.entered == nil
                && isBalanced(block.check)
                && !block.adjustments.contains { $0.verb == .add }
        }
        let quiet = detailed.filter(isQuiet)
        if !quiet.isEmpty {
            blocks.append(.balancedAccounts(quiet.map(\.check)))
        }
        blocks += detailed.filter { !isQuiet($0) }.map { .balanceCheck($0) }
        blocks += balanceAsks.filter { !asked.contains($0.askKey) }.map { .balanceQuestion($0) }
        let unchecked = Int(checks.unchecked) - balanceAsks.count
        if unchecked > 0 {
            blocks.append(.note(
                "\(ImportCopy.countOf(unchecked, "account balance")) couldn't be checked: the statements give nothing to compare with."
            ))
        }
        return blocks
    }

    private static func importance(_ row: ImportStagedRow) -> Int {
        if row.amendRefusal != nil { return 0 }
        if !row.nearMiss.isEmpty { return 1 }
        if row.verb == .delete { return 2 }
        if row.verb == .amend { return 3 }
        if row.kind == .joinedTransfer { return 4 }
        if row.verb == .skip { return 5 }
        return 6
    }

    private static func previewRows(_ rows: [ImportStagedRow]) -> [ImportStagedRow] {
        if rows.count <= inlineRows {
            return rows.sorted { $0.date > $1.date }
        }
        let sorted = rows.sorted { lhs, rhs in
            let left = importance(lhs)
            let right = importance(rhs)
            return left != right ? left < right : lhs.date > rhs.date
        }
        return Array(sorted.prefix(previewRowCount))
    }

    private static func verbCount(_ items: ImportItems, _ verb: ImportRowVerb) -> Int {
        items.rows.filter { $0.verb == verb }.count
    }

    private static func outcome(_ detail: ImportSessionDetail, items: ImportItems) -> OutcomeState {
        let add = verbCount(items, .add)
        let amend = verbCount(items, .amend)
        let delete = verbCount(items, .delete)
        let skip = verbCount(items, .skip)
        let addsOnly = amend == 0 && delete == 0
        let headline = if addsOnly && add > 0 {
            "\(ImportCopy.countOf(add, "transaction")) to add"
        } else if addsOnly {
            "Nothing new to add"
        } else {
            ImportCopy.countOf(add + amend + delete, "change")
        }
        let breakdown: String? = if addsOnly {
            skip > 0 ? "\(ImportCopy.formatCount(skip)) already in your ledger" : nil
        } else {
            changeParts(add: add, amend: amend, delete: delete, skip: skip)
        }
        let dates = items.rows.map(\.date)
        let range = dates.min().map { ImportDates.range($0, dates.max() ?? $0) }
        let scope = [detail.summary.title, range].compactMap { $0 }.joined(separator: " · ")
        let checks = items.balanceChecks?.checks ?? []
        let off = offBalanceCount(items)
        let balance: BalanceLine? = if checks.isEmpty {
            nil
        } else if off == 0 {
            BalanceLine(text: checks.count == 1 ? "Matches the statement" : "Matches the statements", ok: true)
        } else {
            BalanceLine(text: "\(ImportCopy.countOf(off, "balance")) won't match the \(off == 1 ? "statement" : "statements")", ok: false)
        }
        return OutcomeState(headline: headline, breakdown: breakdown, scope: scope, balance: balance)
    }

    private static func changeParts(add: Int, amend: Int, delete: Int, skip: Int) -> String {
        let joined = [
            add > 0 ? "adds \(ImportCopy.formatCount(add))" : nil,
            amend > 0 ? "corrects \(ImportCopy.formatCount(amend))" : nil,
            delete > 0 ? "deletes \(ImportCopy.formatCount(delete))" : nil,
            skip > 0 ? "skips \(ImportCopy.formatCount(skip))" : nil,
        ]
        .compactMap { $0 }
        .joined(separator: " · ")
        return joined.prefix(1).uppercased() + joined.dropFirst()
    }

    private static func receiptLines(_ detail: ImportSessionDetail, applied: AppliedCounts?) -> [String] {
        guard let applied else {
            let recorded = detail.sources.reduce(Int64(0)) { $0 + $1.recordedTransactions }
            return ["\(ImportCopy.countOf(recorded, "transaction")) recorded from \(ImportCopy.sourcesCount(detail.sources))"]
        }
        let lines = [
            applied.added > 0 ? "\(ImportCopy.formatCount(applied.added)) added" : nil,
            applied.corrected > 0 ? "\(ImportCopy.formatCount(applied.corrected)) corrected" : nil,
            applied.deleted > 0 ? "\(ImportCopy.formatCount(applied.deleted)) deleted" : nil,
            applied.skipped > 0 ? "\(ImportCopy.formatCount(applied.skipped)) skipped as already recorded" : nil,
        ].compactMap { $0 }
        return lines.isEmpty ? ["Nothing new was written"] : lines
    }

    private static func hero(_ detail: ImportSessionDetail) -> SessionBlock {
        let failed = detail.files.filter { $0.status == .failed }.count
        if failed > 0 {
            return .hero(
                StatusHeaderState(
                    label: "\(ImportCopy.countOf(failed, "file")) \(failed == 1 ? "needs" : "need") attention",
                    summary: "Retry reading them, or remove the files that aren't statements.",
                    tone: .negative,
                    systemImage: "exclamationmark.triangle",
                    working: false
                ),
                retryCategorizing: false
            )
        }
        if detail.summary.classificationStatus == .failed {
            return .hero(
                StatusHeaderState(
                    label: "Couldn't categorize",
                    summary: "The transactions could not be categorized.",
                    tone: .negative,
                    systemImage: "exclamationmark.circle",
                    working: false,
                    error: detail.classificationError
                ),
                retryCategorizing: true
            )
        }
        let copy = detail.summary.status.copy
        let collecting = detail.summary.status == .collecting
        return .hero(
            StatusHeaderState(
                label: copy.label,
                summary: copy.summary,
                tone: collecting ? .ghost : .brand,
                systemImage: collecting ? "doc.badge.plus" : "hourglass",
                working: !collecting
            ),
            retryCategorizing: false
        )
    }
}
