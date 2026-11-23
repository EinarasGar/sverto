import SvertoCore
import SwiftUI

struct SessionActions {
    let navigate: (DecisionTarget) -> Void
    let openRow: (String) -> Void
    let openAllRows: () -> Void
    let pickAccount: (_ rawAccount: String) -> Void
    let createAccount: (_ rawAccount: String) -> Void
    let searchAsset: (AssetAsk) -> Void
    let reviewGaps: (_ blockId: String) -> Void
    let addFiles: (ImportFileOrigin) -> Void
    let viewTransactions: () -> Void
}

struct SessionBlockView: View {
    private static let maxListedRows = 100

    let block: SessionBlock
    let model: ImportSessionViewModel
    @Binding var expanded: Set<String>
    let actions: SessionActions

    private var enabled: Bool { !model.working }

    private var isExpanded: Binding<Bool> {
        Binding(
            get: { expanded.contains(block.id) },
            set: { isOn in
                if isOn { expanded.insert(block.id) } else { expanded.remove(block.id) }
            }
        )
    }

    var body: some View {
        switch block {
        case let .hero(state, retryCategorizing):
            ImportStatusHeader(state: state, retryLabel: retryCategorizing ? "Retry categorizing" : nil) {
                Task { await model.retryClassification() }
            }
        case let .receipt(lines):
            ImportReceipt(lines: lines, onViewTransactions: actions.viewTransactions)
        case let .questionsHeading(count):
            QuestionsHeading(count: count)
        case let .section(title, count, addAllIds):
            SectionHeader(title, trailing: sectionTrailing(count: count, addAllIds: addAllIds))
                .padding(.top, 12)
        case let .decisionNav(items):
            DecisionNavCard(items: items) { actions.navigate($0.target) }
        case let .outcome(state):
            OutcomeCard(state: state)
        case let .rows(rows, total):
            PreviewRowsCard(rows: rows, total: total, onRowTap: { actions.openRow($0.id) }, onViewAll: actions.openAllRows)
        case let .accountQuestion(question):
            accountQuestion(question)
        case let .assetQuestion(ask):
            assetQuestion(ask)
        case let .balanceQuestion(ask):
            BalanceQuestionCard(accountName: ask.accountName, ticker: ask.ticker, asOfDate: ask.asOfDate, enabled: enabled) { quantity in
                Task { await model.enterBalance(askKey: ask.askKey, quantity: quantity) }
            }
        case let .files(collapsible):
            if let detail = model.detail {
                ImportFilesSection(
                    sources: detail.sources,
                    uploads: model.uploads,
                    acceptsFiles: detail.summary.status != .completed,
                    enabled: enabled,
                    collapsible: collapsible,
                    isExpanded: isExpanded,
                    onAddFiles: actions.addFiles,
                    onRetryFile: { id in model.aiConsent.perform { Task { await model.retryFile(fileId: id) } } },
                    onRemoveFile: { id in Task { await model.removeFile(fileId: id) } },
                    onDateOrder: { id, order in model.aiConsent.perform { Task { await model.setDateOrder(fileId: id, order: order) } } },
                    onRetryUpload: { id in Task { await model.retryUpload(localId: id) } },
                    onDismissUpload: model.dismissUpload
                )
                .padding(.top, collapsible ? 12 : 0)
            }
        case let .pending(count):
            ExpandableCard(
                title: "\(ImportCopy.countOf(count, "transaction")) \(count == 1 ? "waits" : "wait") on your answers",
                isExpanded: isExpanded
            ) {
                ForEach(Array(model.pending.prefix(Self.maxListedRows).enumerated()), id: \.offset) { _, row in
                    SimpleRow(
                        title: row.description ?? row.accountRef,
                        detail: "\(ImportDates.date(row.date)) · \(row.accountRef)",
                        trailing: row.amountsDisplay
                    )
                }
                if model.pending.count > Self.maxListedRows {
                    RowDivider()
                    DetailLine("Showing \(Self.maxListedRows) of \(ImportCopy.formatCount(model.pending.count)) transactions")
                        .padding(Layout.cardPadding)
                }
            }
        case let .duplicate(row, progress):
            PossibleDuplicateCard(
                row: row,
                progress: progress,
                selectedId: model.duplicateChoices[row.id],
                enabled: enabled,
                onSelect: { model.chooseDuplicate(rowId: row.id, choice: $0) },
                onConfirm: { Task { await model.confirmDuplicate(rowId: row.id) } }
            )
        case let .startingBalance(row):
            StartingBalanceCard(
                row: row,
                enabled: enabled,
                onAdd: { Task { await model.setVerb(itemIds: [row.id], verb: .add) } },
                onLeaveOut: { Task { await model.setVerb(itemIds: [row.id], verb: .skip) } }
            )
        case let .balancedAccounts(checks):
            BalancedAccountsCard(checks: checks)
        case let .balanceCheck(check):
            BalanceCheckCard(
                block: check,
                enabled: enabled,
                onEnterBalance: { askKey, quantity in Task { await model.enterBalance(askKey: askKey, quantity: quantity) } },
                onReviewGaps: { actions.reviewGaps(block.id) }
            )
        case let .note(text):
            DetailLine(text)
                .padding(.horizontal, 4)
        case let .unstageable(count):
            ExpandableCard(title: "\(ImportCopy.countOf(count, "transaction")) couldn't be imported", isExpanded: isExpanded) {
                ForEach(Array((model.items?.unstageable ?? []).enumerated()), id: \.offset) { _, row in
                    SimpleRow(title: row.description ?? "No description", detail: "\(ImportDates.date(row.date)) · \(row.reason)")
                }
            }
        }
    }

    private func sectionTrailing(count: String?, addAllIds: [String]) -> (() -> AnyView)? {
        if !addAllIds.isEmpty {
            return {
                AnyView(
                    Button("Add all") {
                        Task { await model.setVerb(itemIds: addAllIds, verb: .add) }
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(!enabled)
                    .accessibilityLabel("Add all \(addAllIds.count) starting balances")
                )
            }
        }
        if let count {
            return { AnyView(CountLabel(count)) }
        }
        return nil
    }

    private func accountQuestion(_ question: AccountQuestion) -> some View {
        let raw = question.rawAccount
        let separate = model.separateAccounts.contains(raw)
        let candidates = question.candidates
        let chosen = model.accountChoices[raw]
        let extra = chosen
            .flatMap { id in candidates.contains { $0.id == id } ? nil : model.accounts?.first { $0.id == id } }
            .map { QuestionOption(id: $0.id, title: $0.name) }
        let selected = chosen ?? candidates.first?.id
        let labels = Dictionary((model.detail?.sources ?? []).map { ($0.id, $0.label) }, uniquingKeysWith: { first, _ in first })
        var seen = Set<String>()
        let sourceNames = (question.coversAll ? question.asks : question.targets(separate: separate))
            .compactMap { labels[$0.sourceId] }
            .filter { !$0.isEmpty && seen.insert($0).inserted }
        return AccountQuestionCard(
            rawAccount: raw,
            sourceNames: sourceNames,
            options: candidates.map { QuestionOption(id: $0.id, title: $0.name) } + (extra.map { [$0] } ?? []),
            selectedId: selected,
            canExclude: question.asks.allSatisfy(\.canExclude),
            sameAccount: question.splittable ? !separate : nil,
            enabled: enabled,
            onSelect: { model.chooseAccount(rawAccount: raw, accountId: $0) },
            onChooseAnother: { actions.pickAccount(raw) },
            onCreateAccount: { actions.createAccount(raw) },
            onSameAccountChange: { model.setSameAccount(rawAccount: raw, same: $0) },
            onExclude: { Task { await model.answerAccount(question, accountId: nil) } },
            onLink: {
                guard let selected else { return }
                Task { await model.answerAccount(question, accountId: selected) }
            }
        )
    }

    private func assetQuestion(_ ask: AssetAsk) -> some View {
        let chosen = model.assetChoices[ask.askKey]
        let extra = chosen.flatMap { pick in ask.candidates.contains { $0.id == pick.id } ? nil : pick }
        let options = (ask.candidates + (extra.map { [$0] } ?? [])).map { QuestionOption(id: String($0.id), title: $0.ticker, subtitle: $0.name) }
        let selected = chosen?.id ?? ask.candidates.first?.id
        return AssetQuestionCard(
            raw: ask.raw,
            options: options,
            selectedId: selected.map { String($0) },
            canExclude: ask.canExclude,
            enabled: enabled,
            onSelect: { id in
                if let asset = ask.candidates.first(where: { String($0.id) == id }) ?? chosen {
                    model.chooseAsset(askKey: ask.askKey, asset: asset)
                }
            },
            onSearch: { actions.searchAsset(ask) },
            onLeaveOut: { Task { await model.answerAsset(askKey: ask.askKey, assetId: nil) } },
            onLink: {
                guard let selected else { return }
                Task { await model.answerAsset(askKey: ask.askKey, assetId: selected) }
            }
        )
    }
}
