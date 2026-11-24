import Foundation
import Observation
import SvertoCore

@Observable
final class CreateTransactionGroupViewModel {
    private(set) var formState = GroupFormState()
    private(set) var submitState: SubmitState = .idle
    private(set) var isLoading = true
    private(set) var loadError: String?
    private(set) var errorMessage: String?
    private(set) var errorRevision = 0
    private(set) var quickUploadId: String?
    let categories: CategoryFieldController
    let correction: ProposalCorrectionController
    let ledgerMatch: LedgerMatchController

    private let store: AppStore
    private var editGroupId: String?
    private var fromSelection = false
    private var initialized = false
    private var initialSource: GroupFormSource?
    private var initialFormState: GroupFormState?

    var isBusy: Bool { submitState == .submitting || correction.state == .sending }
    var canEdit: Bool { initialFormState != nil && !isLoading && loadError == nil && submitState == .idle && !isBusy }
    var canRetryLoading: Bool { initialSource != nil && loadError != nil }
    var hasUnsavedChanges: Bool {
        guard let initialFormState else { return false }
        return formState != initialFormState
    }

    init(store: AppStore) {
        self.store = store
        categories = CategoryFieldController(store: store)
        correction = ProposalCorrectionController(store: store)
        ledgerMatch = LedgerMatchController(store: store)
    }

    func initNew() {
        guard beginInit(source: .new) else { return }
        formState = GroupFormState(date: nowEpochSeconds)
        initialFormState = formState
        isLoading = false
    }

    func initForEdit(group: TransactionListItem) async {
        guard beginInit(source: .edit(group)) else { return }
        await loadInitialState()
    }

    func initFromSelection(transactionIds: [String]) async {
        guard beginInit(source: .selection(transactionIds)) else { return }
        await loadInitialState()
    }

    func initFromProposal(quickUploadId: String) async {
        guard beginInit(source: .proposal(quickUploadId)) else { return }
        await loadInitialState()
    }

    func reportMissingGroup() {
        guard !initialized else { return }
        initialized = true
        isLoading = false
        loadError = "This group could not be opened. Go back and reopen it."
    }

    func retryLoading() async {
        guard canRetryLoading, !isLoading else { return }
        await loadInitialState()
    }

    private func loadInitialState() async {
        guard let initialSource else { return }
        isLoading = true
        loadError = nil
        defer { isLoading = false }
        do {
            switch initialSource {
            case .new:
                formState = GroupFormState(date: nowEpochSeconds)
            case let .edit(group):
                editGroupId = group.id
                let children = try await loadChildren(ids: group.children.map(\.id))
                formState = GroupFormState(
                    date: group.date,
                    description: group.description,
                    categoryId: group.categoryId,
                    categoryName: group.categoryName,
                    transactions: children
                )
                categories.suggestions.markCommitted(text: group.description)
            case let .selection(transactionIds):
                fromSelection = true
                let children = try await loadChildren(ids: transactionIds)
                formState = GroupFormState(date: nowEpochSeconds, transactions: children)
            case let .proposal(uploadId):
                async let matchesLoaded: Void = ledgerMatch.load(uploadId: uploadId)
                let detail = try await store.getQuickUploadDetail(uploadId: uploadId)
                guard let proposal = ProposalMapper.groupFormState(
                    proposalData: detail.proposalData ?? "{}",
                    lookupTables: detail.lookupTables
                ) else {
                    loadError = "The receipt proposal could not be loaded. Try again."
                    return
                }
                formState = proposal
                categories.suggestions.markCommitted(text: proposal.description)
                quickUploadId = uploadId
                await matchesLoaded
            }
            initialFormState = formState
        } catch {
            loadError = error.friendlyMessage
        }
    }

    func sendCorrection(_ message: String) {
        guard canEdit, let uploadId = quickUploadId else { return }
        correction.send(message, uploadId: uploadId, expecting: .transactionGroup) { proposalData, lookupTables in
            ProposalMapper.groupFormState(proposalData: proposalData, lookupTables: lookupTables)
        } apply: { [weak self] corrected in
            guard let self else { return }
            formState = corrected
            categories.suggestions.markCommitted(text: corrected.description)
            Task { await self.ledgerMatch.load(uploadId: uploadId) }
        }
    }

    func updateDate(_ date: Date) {
        formState.date = Int64(date.timeIntervalSince1970)
    }

    func updateDescription(_ value: String) {
        categories.updateDescription(value, in: &formState)
    }

    func onDescriptionCommitted() async {
        await categories.commitDescription(of: self, at: \.formState)
    }

    func applySuggestion() {
        categories.applySuggestion(in: &formState)
    }

    func selectCategory(_ item: CategoryItem) {
        categories.select(item, in: &formState)
    }

    func addTransaction(_ item: GroupTransactionItem) {
        formState.transactions.append(IdentifiedGroupTransaction(item))
    }

    func updateTransaction(index: Int, item: GroupTransactionItem) {
        guard formState.transactions.indices.contains(index) else { return }
        formState.transactions[index].item = item
    }

    func removeTransaction(id: UUID) {
        formState.transactions.removeAll { $0.id == id }
    }

    func submit() async {
        guard canEdit else { return }
        errorMessage = nil
        let state = formState
        if let validationMessage = validationMessage(for: state) {
            reportError(validationMessage)
            return
        }
        guard let date = state.date, let categoryId = state.categoryId else { return }
        if let uploadId = quickUploadId, let mergeTargetId = ledgerMatch.mergeTargetId {
            let lines = state.transactions.map(\.item.input)
            guard lines.allSatisfy({ $0.typeKey == TransactionTypeConfig.regularApiType }) else {
                reportError("Only spending lines can be merged into a transaction")
                return
            }
            let receipt = QuickUploadReceipt(
                date: date,
                description: state.description,
                categoryId: categoryId,
                lines: lines.map { line in
                    QuickUploadReceiptLine(description: line.description, categoryId: line.categoryId ?? categoryId, amount: line.primaryAmount)
                }
            )
            submitState = .submitting
            do {
                try await store.mergeQuickUpload(uploadId: uploadId, transactionId: mergeTargetId, receipt: receipt)
                submitState = .success
            } catch {
                reportError(error.friendlyMessage)
                submitState = .idle
            }
            return
        }
        submitState = .submitting
        let input = CreateTransactionGroupInput(
            date: date,
            description: state.description,
            categoryId: categoryId,
            transactions: state.transactions.map(\.item.input)
        )
        do {
            if let groupId = editGroupId {
                try await store.updateTransactionGroup(groupId: groupId, input: input)
            } else if fromSelection {
                try await store.groupIndividualTransactions(input: input)
            } else {
                try await store.createTransactionGroup(input: input)
            }
            if let uploadId = quickUploadId {
                await store.completeQuickUpload(uploadId: uploadId, accepted: true)
                quickUploadId = nil
            }
            submitState = .success
        } catch {
            reportError(error.friendlyMessage)
            submitState = .idle
        }
    }

    private var nowEpochSeconds: Int64 {
        Int64(Date().timeIntervalSince1970)
    }

    private func reportError(_ message: String) {
        errorMessage = message
        errorRevision += 1
    }

    private func beginInit(source: GroupFormSource) -> Bool {
        guard !initialized else { return false }
        initialized = true
        initialSource = source
        editGroupId = nil
        fromSelection = false
        submitState = .idle
        errorMessage = nil
        isLoading = true
        return true
    }

    private func validationMessage(for state: GroupFormState) -> String? {
        if state.date == nil { return "Date is required" }
        if state.categoryId == nil { return "Category is required" }
        if state.transactions.isEmpty { return "Add at least one transaction" }
        if state.description.isBlank { return "Description is required" }
        return nil
    }

    private func loadChildren(ids: [String]) async throws -> [IdentifiedGroupTransaction] {
        let tasks = ids.map { id in Task { try await self.loadGroupItem(transactionId: id) } }
        do {
            var items: [IdentifiedGroupTransaction] = []
            for task in tasks {
                items.append(IdentifiedGroupTransaction(try await task.value))
            }
            return items
        } catch {
            tasks.forEach { $0.cancel() }
            throw error
        }
    }

    private func loadGroupItem(transactionId: String) async throws -> GroupTransactionItem {
        let editable = try await store.getEditableTransaction(txId: transactionId)
        let typeLabel = TransactionTypeCatalog.label(for: apiTypeToConfigKey(editable.typeKey))
        let input = CreateTransactionInput(
            transactionId: transactionId,
            typeKey: editable.typeKey,
            date: editable.date,
            primaryEntryId: editable.primaryEntryId,
            primaryAccountId: editable.primaryAccountId,
            primaryAssetId: editable.primaryAssetId,
            primaryAmount: editable.primaryAmount,
            secondaryEntryId: editable.secondaryEntryId,
            secondaryAccountId: editable.secondaryAccountId,
            secondaryAssetId: editable.secondaryAssetId,
            secondaryAmount: editable.secondaryAmount,
            originAssetId: editable.originAssetId,
            categoryId: editable.categoryId,
            description: editable.description.isBlank ? nil : editable.description
        )
        return GroupTransactionItem(
            input: input,
            descriptionDisplay: editable.description.isBlank ? typeLabel : editable.description,
            typeLabel: typeLabel,
            amountDisplay: EditableAmount.string(editable.primaryAmount),
            accountName: editable.primaryAccountName,
            assetDisplay: editable.primaryAssetDisplay,
            categoryName: editable.categoryName
        )
    }
}
