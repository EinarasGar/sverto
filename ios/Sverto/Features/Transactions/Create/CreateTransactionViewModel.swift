import Foundation
import Observation
import SvertoCore

@Observable
final class CreateTransactionViewModel {
    private(set) var typeKey: String
    private(set) var accounts: [AccountItem] = []
    private(set) var assetSearchState: AssetSearchState = .idle
    private(set) var formState = TransactionFormState()
    private(set) var submitState: SubmitState = .idle
    private(set) var submittedTransaction: TransactionListItem?
    private(set) var groupResult: GroupTransactionItem?
    private(set) var isLoading = true
    private(set) var loadError: String?
    private(set) var errorMessage: String?
    private(set) var errorRevision = 0
    private(set) var quickUploadId: String?
    private(set) var hasAttemptedSubmit = false

    let mode: CreateTransactionMode
    let categories: CategoryFieldController
    let correction: ProposalCorrectionController
    let ledgerMatch: LedgerMatchController

    var config: TransactionTypeConfig { .forKey(typeKey) }
    var isBusy: Bool { submitState == .submitting || correction.state == .sending }
    var canEdit: Bool { initialFormState != nil && !isLoading && loadError == nil && submitState == .idle && !isBusy }
    var hasUnsavedChanges: Bool {
        guard let initialFormState, let initialTypeKey else { return false }
        return formState != initialFormState || typeKey != initialTypeKey
    }

    func validationMessage(for field: RequiredTransactionField) -> String? {
        guard hasAttemptedSubmit, missingFields.contains(field) else { return nil }
        return field.message
    }

    var offersLedgerMatch: Bool { typeKey == TransactionTypeCatalog.regular.key }
    var mergeTargetId: String? { offersLedgerMatch ? ledgerMatch.mergeTargetId : nil }

    private var missingFields: Set<RequiredTransactionField> {
        var fields = TransactionFormValidation.missingFields(in: formState, config: config)
        if mergeTargetId != nil {
            fields.remove(.primaryAccount)
        }
        return fields
    }

    private let store: AppStore
    private var editTransactionId: String?
    private var started = false
    private var assetSearchTask: Task<Void, Never>?
    private var assetSearchGeneration = UUID()
    private var assetSearchQuery = ""
    private var initialFormState: TransactionFormState?
    private var initialTypeKey: String?

    init(store: AppStore, typeKey: String, mode: CreateTransactionMode) {
        self.store = store
        self.typeKey = typeKey
        self.mode = mode
        categories = CategoryFieldController(store: store)
        correction = ProposalCorrectionController(store: store)
        ledgerMatch = LedgerMatchController(store: store)
    }

    func start() async {
        guard !started else { return }
        started = true
        await loadInitialState()
    }

    func retryLoading() async {
        guard loadError != nil, !isLoading else { return }
        await loadInitialState()
    }

    private func loadInitialState() async {
        isLoading = true
        loadError = nil
        defer { isLoading = false }
        do {
            accounts = try await store.getAccountsList()
            switch mode {
            case .new, .groupAdd:
                formState = TransactionFormState(
                    date: Int64(Date().timeIntervalSince1970),
                    primaryEntry: EntryFormState(amount: config.entryMode.primarySign.blankAmount)
                )
            case let .edit(transactionId):
                let editable = try await store.getEditableTransaction(txId: transactionId)
                editTransactionId = transactionId
                typeKey = apiTypeToConfigKey(editable.typeKey)
                formState = editable.formState(transactionId: transactionId)
                categories.suggestions.markCommitted(text: editable.description)
            case let .proposal(uploadId):
                async let matchesLoaded: Void = ledgerMatch.load(uploadId: uploadId)
                let detail = try await store.getQuickUploadDetail(uploadId: uploadId)
                guard let proposed = ProposalMapper.formState(proposalData: detail.proposalData ?? "{}", lookupTables: detail.lookupTables) else {
                    loadError = "The receipt proposal could not be loaded. Try again."
                    return
                }
                formState = proposed
                categories.suggestions.markCommitted(text: proposed.description)
                quickUploadId = uploadId
                await matchesLoaded
            case let .groupEdit(item):
                initForGroupEdit(item)
            }
            initialFormState = formState
            initialTypeKey = typeKey
        } catch {
            loadError = error.friendlyMessage
        }
    }

    private func initForGroupEdit(_ item: GroupTransactionItem) {
        let input = item.input
        editTransactionId = input.transactionId
        typeKey = apiTypeToConfigKey(input.typeKey)
        formState = TransactionFormState(
            transactionId: input.transactionId,
            date: input.date,
            description: input.description ?? "",
            categoryId: input.categoryId,
            categoryName: item.categoryName,
            originAssetId: input.originAssetId,
            primaryEntry: EntryFormState(
                entryId: input.primaryEntryId,
                accountId: input.primaryAccountId,
                accountName: item.accountName,
                assetId: input.primaryAssetId,
                assetDisplay: item.assetDisplay,
                amount: EditableAmount.string(input.primaryAmount)
            ),
            secondaryEntry: EntryFormState(
                entryId: input.secondaryEntryId,
                accountId: input.secondaryAccountId,
                assetId: input.secondaryAssetId,
                amount: input.secondaryAmount.map(EditableAmount.string) ?? ""
            )
        )
        categories.suggestions.markCommitted(text: input.description ?? "")
    }

    func changeType(_ newTypeKey: String) {
        guard typeKey != newTypeKey else { return }
        let from = config
        let to = TransactionTypeConfig.forKey(newTypeKey)
        typeKey = newTypeKey
        categories.suggestions.reset()
        formState = TypeConversion.convert(formState, from: from, to: to)
    }

    func searchAssets(_ query: String) {
        resetAssetSearch()
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        assetSearchQuery = query
        assetSearchState = .loading
        let generation = assetSearchGeneration
        assetSearchTask = Task { [store] in
            do {
                try await Task.sleep(for: .milliseconds(300))
                let results = try await store.searchAssets(query: query)
                guard !Task.isCancelled, assetSearchGeneration == generation else { return }
                assetSearchState = .results(results)
            } catch {
                guard !Task.isCancelled, assetSearchGeneration == generation else { return }
                assetSearchState = .failed(error.friendlyMessage)
            }
        }
    }

    func retryAssetSearch() {
        searchAssets(assetSearchQuery)
    }

    func resetAssetSearch() {
        assetSearchTask?.cancel()
        assetSearchTask = nil
        assetSearchGeneration = UUID()
        assetSearchQuery = ""
        assetSearchState = .idle
    }

    func onDescriptionCommitted() async {
        await categories.commitDescription(of: self, at: \.formState)
    }

    func applySuggestion() {
        categories.applySuggestion(in: &formState)
    }

    func updateDate(_ date: Date) {
        formState.date = Int64(date.timeIntervalSince1970)
    }

    func updateDescription(_ value: String) {
        categories.updateDescription(value, in: &formState)
    }

    func selectCategory(_ item: CategoryItem) {
        categories.select(item, in: &formState)
    }

    func selectOriginAsset(_ item: AssetItem) {
        formState.originAssetId = item.id
        formState.originAssetDisplay = item.display
        resetAssetSearch()
    }

    func updatePrimaryAccount(_ item: AccountItem) {
        formState.primaryEntry.accountId = item.id
        formState.primaryEntry.accountName = item.name
        applySuggestedCurrency(from: item, target: .primary)
    }

    func updatePrimaryAsset(_ item: AssetItem) {
        formState.primaryEntry.assetId = item.id
        formState.primaryEntry.assetDisplay = item.display
        resetAssetSearch()
    }

    func updatePrimaryAmount(_ value: String) {
        formState.primaryEntry.amount = value
    }

    func updateSecondaryAccount(_ item: AccountItem) {
        formState.secondaryEntry.accountId = item.id
        formState.secondaryEntry.accountName = item.name
        applySuggestedCurrency(from: item, target: .secondary)
    }

    func updateSecondaryAsset(_ item: AssetItem) {
        formState.secondaryEntry.assetId = item.id
        formState.secondaryEntry.assetDisplay = item.display
        resetAssetSearch()
    }

    func updateSecondaryAmount(_ value: String) {
        formState.secondaryEntry.amount = value
    }

    func submit() async {
        guard canEdit else { return }
        errorMessage = nil
        let config = self.config
        hasAttemptedSubmit = true
        guard missingFields.isEmpty else {
            reportError("Check the highlighted fields before saving.")
            return
        }
        if let quickUploadId, let mergeTargetId {
            guard let date = formState.date,
                  let amount = EditableAmount.signed(formState.primaryEntry.amount, sign: config.entryMode.primarySign) else {
                reportError("Missing required fields")
                return
            }
            guard let categoryId = formState.categoryId else {
                reportError("Choose a category before merging the receipt.")
                return
            }
            let description = formState.description.isBlank ? nil : formState.description
            let receipt = QuickUploadReceipt(
                date: date,
                description: description,
                categoryId: categoryId,
                lines: [QuickUploadReceiptLine(description: description, categoryId: categoryId, amount: amount)]
            )
            submitState = .submitting
            do {
                try await store.mergeQuickUpload(uploadId: quickUploadId, transactionId: mergeTargetId, receipt: receipt)
                submitState = .success
            } catch {
                reportError(error.friendlyMessage)
                submitState = .idle
            }
            return
        }
        guard let input = buildInput(config: config) else {
            reportError("Missing required fields")
            return
        }
        submitState = .submitting
        if mode.isGroupMode {
            groupResult = GroupTransactionItem(
                input: input,
                descriptionDisplay: formState.description.isBlank ? config.label : formState.description,
                typeLabel: config.label,
                amountDisplay: EditableAmount.string(input.primaryAmount),
                accountName: formState.primaryEntry.accountName,
                assetDisplay: formState.primaryEntry.assetDisplay,
                categoryName: formState.categoryName
            )
            submitState = .success
            return
        }
        do {
            if let editTransactionId {
                submittedTransaction = try await store.updateIndividualTransaction(txId: editTransactionId, input: input)
            } else {
                try await store.createIndividualTransaction(input: input)
                submittedTransaction = nil
            }
            if let quickUploadId {
                await store.completeQuickUpload(uploadId: quickUploadId, accepted: true)
                self.quickUploadId = nil
            }
            submitState = .success
        } catch {
            reportError(error.friendlyMessage)
            submitState = .idle
        }
    }

    func sendCorrection(_ message: String) {
        guard canEdit, let uploadId = quickUploadId else { return }
        correction.send(message, uploadId: uploadId, expecting: .transaction) { proposalData, lookupTables in
            ProposalMapper.formState(proposalData: proposalData, lookupTables: lookupTables)
        } apply: { [weak self] corrected in
            guard let self else { return }
            formState = corrected
            categories.suggestions.markCommitted(text: corrected.description)
            Task { await self.ledgerMatch.load(uploadId: uploadId) }
        }
    }

    private func reportError(_ message: String) {
        errorMessage = message
        errorRevision += 1
    }

    private func applySuggestedCurrency(from item: AccountItem, target: AccountPickerTarget) {
        guard let currency = item.suggestedCurrency else { return }
        switch config.entryMode {
        case let .single(mode):
            if mode.isCash, target == .primary {
                formState.primaryEntry = formState.primaryEntry.withAsset(currency)
            }
        case let .dual(mode):
            let fillsPrimary = mode.primaryIsCash && (target == .primary || mode.sameAccount)
            let fillsSecondary = mode.secondaryIsCash && (target == .secondary || mode.sameAccount || mode.sameAsset)
            if fillsPrimary || (fillsSecondary && mode.sameAsset) {
                formState.primaryEntry = formState.primaryEntry.withAsset(currency)
            }
            if fillsSecondary, !mode.sameAsset {
                formState.secondaryEntry = formState.secondaryEntry.withAsset(currency)
            }
        }
    }

    private func buildInput(config: TransactionTypeConfig) -> CreateTransactionInput? {
        let state = formState
        guard let date = state.date,
              let primaryAccountId = state.primaryEntry.accountId,
              let primaryAssetId = state.primaryEntry.assetId,
              let primaryAmount = EditableAmount.signed(state.primaryEntry.amount, sign: config.entryMode.primarySign) else { return nil }

        var secondaryAccountId: String?
        var secondaryAssetId: Int32?
        var secondaryAmount: Double?
        if case let .dual(mode) = config.entryMode {
            let account = mode.sameAccount ? primaryAccountId : state.secondaryEntry.accountId
            let asset = mode.sameAsset ? primaryAssetId : state.secondaryEntry.assetId
            let amountRaw = mode.sharedAmount ? state.primaryEntry.amount : state.secondaryEntry.amount
            guard let account, let asset, let amount = EditableAmount.signed(amountRaw, sign: mode.secondarySign) else { return nil }
            secondaryAccountId = account
            secondaryAssetId = asset
            secondaryAmount = amount
        }

        return CreateTransactionInput(
            transactionId: state.transactionId,
            typeKey: config.apiType,
            date: date,
            primaryEntryId: state.primaryEntry.entryId,
            primaryAccountId: primaryAccountId,
            primaryAssetId: primaryAssetId,
            primaryAmount: primaryAmount,
            secondaryEntryId: state.secondaryEntry.entryId,
            secondaryAccountId: secondaryAccountId,
            secondaryAssetId: secondaryAssetId,
            secondaryAmount: secondaryAmount,
            originAssetId: state.originAssetId,
            categoryId: state.categoryId,
            description: state.description.isBlank ? nil : state.description
        )
    }
}

private extension EditableTransaction {
    func formState(transactionId: String) -> TransactionFormState {
        TransactionFormState(
            transactionId: transactionId,
            date: date,
            description: description,
            categoryId: categoryId,
            categoryName: categoryName,
            originAssetId: originAssetId,
            originAssetDisplay: originAssetDisplay,
            primaryEntry: EntryFormState(
                entryId: primaryEntryId,
                accountId: primaryAccountId,
                accountName: primaryAccountName,
                assetId: primaryAssetId,
                assetDisplay: primaryAssetDisplay,
                amount: EditableAmount.string(primaryAmount)
            ),
            secondaryEntry: EntryFormState(
                entryId: secondaryEntryId,
                accountId: secondaryAccountId,
                accountName: secondaryAccountName ?? "",
                assetId: secondaryAssetId,
                assetDisplay: secondaryAssetDisplay ?? "",
                amount: secondaryAmount.map(EditableAmount.string) ?? ""
            )
        )
    }
}
