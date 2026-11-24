import SvertoCore
import SwiftUI

struct CreateTransactionScreen: View {
    let typeKey: String
    let quickUploadId: String?
    var editTransactionId: String? = nil
    var groupChildMode: GroupChildMode? = nil

    @Environment(AppServices.self) private var services
    @Environment(ShellState.self) private var shell

    private var resolvedMode: CreateTransactionMode? {
        switch groupChildMode {
        case .add:
            return .groupAdd
        case let .edit(index):
            guard shell.groupChildDrafts.indices.contains(index) else { return nil }
            return .groupEdit(shell.groupChildDrafts[index])
        case nil:
            if let quickUploadId { return .proposal(quickUploadId: quickUploadId) }
            if let editTransactionId { return .edit(transactionId: editTransactionId) }
            return .new
        }
    }

    var body: some View {
        if let mode = resolvedMode {
            CreateTransactionForm(store: services.store, typeKey: typeKey, routeQuickUploadId: quickUploadId, mode: mode)
        } else {
            Color.clear
                .task { shell.pop() }
        }
    }
}

private struct CreateTransactionForm: View {
    @State private var model: CreateTransactionViewModel
    @State private var presentation = FormPresentation()
    @State private var showsDiscardConfirmation = false
    @FocusState private var focusedField: FormField?
    @Environment(ShellState.self) private var shell
    @Environment(TransactionsViewModel.self) private var transactions
    @Environment(QuickUploadViewModel.self) private var quickUploads

    private let routeQuickUploadId: String?

    init(store: AppStore, typeKey: String, routeQuickUploadId: String?, mode: CreateTransactionMode) {
        self.routeQuickUploadId = routeQuickUploadId
        _model = State(initialValue: CreateTransactionViewModel(store: store, typeKey: typeKey, mode: mode))
    }

    private var config: TransactionTypeConfig { model.config }
    private var correctionUploadId: String? { model.quickUploadId ?? routeQuickUploadId }
    private var isSubmitting: Bool { model.submitState == .submitting }

    var body: some View {
        @Bindable var presentation = presentation
        ScrollViewReader { proxy in
            Form {
                if let message = model.loadError {
                    Section {
                        FormErrorRow(message: message)
                        Button("Retry") { Task { await model.retryLoading() } }
                    }
                } else {
                    if let message = model.errorMessage {
                        Section {
                            FormErrorRow(message: message)
                        }
                        .id("transaction-form-error")
                    }
                    Group {
                        if model.offersLedgerMatch {
                            LedgerMatchSection(controller: model.ledgerMatch)
                        }
                        Section {
                            DateChip(dateEpochSeconds: model.formState.date) { model.updateDate($0) }
                            TransactionFieldFeedback(message: model.validationMessage(for: .date))
                        }
                        switch config.entryMode {
                        case let .single(mode):
                            SingleEntrySections(mode: mode, model: model, presentation: presentation, focus: $focusedField)
                        case let .dual(mode):
                            DualEntrySections(mode: mode, model: model, presentation: presentation, focus: $focusedField)
                        }
                        if correctionUploadId != nil {
                            CorrectionInput(state: model.correction.state, focus: $focusedField, onSend: model.sendCorrection)
                        }
                    }
                    .disabled(!model.canEdit)
                }
            }
            .disabled(model.isBusy)
            .scrollDismissesKeyboard(.interactively)
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarBackButtonHidden()
            .toolbar(.hidden, for: .tabBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: requestCancel)
                        .tint(.primary)
                        .disabled(model.isBusy)
                }
                ToolbarItem(placement: .principal) {
                    TypeTitleButton(label: config.label) { presentation.isTypePickerPresented = true }
                        .disabled(!model.canEdit)
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button {
                        focusedField = nil
                        Task { await model.submit() }
                    } label: {
                        if isSubmitting {
                            ProgressView()
                        } else {
                            Text("Save")
                        }
                    }
                    .disabled(!model.canEdit)
                    .accessibilityLabel(isSubmitting ? "Saving transaction" : model.mode.saveTitle)
                }
                ToolbarItemGroup(placement: .keyboard) {
                    Spacer()
                    Button("Done") { focusedField = nil }
                }
            }
            .overlay {
                if model.isLoading {
                    LoadingOverlay()
                }
            }
            .task { await model.start() }
            .onChange(of: model.typeKey) { _, _ in
                if case .single = config.entryMode {
                    focusedField = .primaryAmount
                }
            }
            .onChange(of: model.errorRevision) { _, _ in
                guard let message = model.errorMessage else { return }
                AccessibilityNotification.Announcement(message).post()
                proxy.scrollTo("transaction-form-error", anchor: .top)
            }
            .onChange(of: model.loadError) { _, message in
                if let message { AccessibilityNotification.Announcement(message).post() }
            }
            .confirmationDialog("Discard changes?", isPresented: $showsDiscardConfirmation, titleVisibility: .visible) {
                Button("Discard Changes", role: .destructive, action: discardAndClose)
                Button("Keep Editing", role: .cancel) { }
            } message: {
                Text("Your changes to this transaction will be lost.")
            }
            .onChange(of: model.submitState) { _, state in
                if state == .success { finish() }
            }
            .onChange(of: model.correction.typeChange) { _, change in
                guard let change else { return }
                model.correction.consumeTypeChange()
                followCorrectionTypeChange(change)
            }
            .sensoryFeedback(.success, trigger: model.submitState == .success)
            .sheet(item: $presentation.accountTarget) { target in
                AccountPickerSheet(accounts: model.accounts, selectedAccountId: selectedAccountId(for: target)) { account in
                    switch target {
                    case .primary: model.updatePrimaryAccount(account)
                    case .secondary: model.updateSecondaryAccount(account)
                    }
                }
            }
            .sheet(item: $presentation.assetTarget, onDismiss: model.resetAssetSearch) { target in
                AssetSearchScene(
                    title: target == .origin ? "Origin asset" : "Asset",
                    state: model.assetSearchState,
                    onQueryChange: model.searchAssets,
                    onRetry: model.retryAssetSearch
                ) { asset in
                    switch target {
                    case .primary: model.updatePrimaryAsset(asset)
                    case .secondary: model.updateSecondaryAsset(asset)
                    case .origin: model.selectOriginAsset(asset)
                    }
                }
            }
            .sheet(isPresented: $presentation.isCategorySearchPresented) {
                CategorySearchScene(results: model.categories.results, isLoading: model.categories.isLoading, onQueryChange: { model.categories.filter($0) }) { category in
                    model.selectCategory(category)
                }
            }
            .sheet(isPresented: $presentation.isTypePickerPresented) {
                TransactionTypePickerSheet(
                    title: "Change type",
                    subtitle: "Pick a different transaction type",
                    showGroupOption: false,
                    selectedTypeKey: model.typeKey,
                    onSelectType: { model.changeType($0) }
                )
            }
            .aiConsentSheet(model.correction.aiConsent)
        }
    }

    private func requestCancel() {
        guard !model.isBusy else { return }
        focusedField = nil
        if model.hasUnsavedChanges {
            showsDiscardConfirmation = true
        } else {
            discardAndClose()
        }
    }

    private func discardAndClose() {
        guard !model.isBusy else { return }
        if model.mode.isGroupMode {
            shell.editingGroupIndex = nil
        }
        shell.pop()
    }

    private func selectedAccountId(for target: AccountPickerTarget) -> String? {
        switch target {
        case .primary: model.formState.primaryEntry.accountId
        case .secondary: model.formState.secondaryEntry.accountId
        }
    }

    private func finish() {
        switch model.mode {
        case .groupAdd, .groupEdit:
            shell.pendingGroupTransaction = model.groupResult
        case let .edit(transactionId):
            if let updated = model.submittedTransaction {
                let isInGroup = shell.transactionCache[transactionId]?.isInGroup ?? false
                shell.transactionCache[transactionId] = TransactionDetailState(transaction: updated, isInGroup: isInGroup)
            }
            refreshTransactions()
        case .new:
            refreshTransactions()
        case let .proposal(uploadId):
            refreshTransactions()
            if model.mergeTargetId == nil {
                Task { await quickUploads.complete(id: uploadId, accepted: true) }
            }
        }
        shell.pop()
    }

    private func refreshTransactions() {
        Task { await transactions.refresh() }
    }

    private func followCorrectionTypeChange(_ change: CorrectionTypeChange) {
        shell.pop()
        switch change.newProposalType {
        case .transactionGroup:
            shell.push(.createTransactionGroup(quickUploadId: change.quickUploadId, fromSelection: false))
        case .transaction:
            shell.push(.createTransaction(typeKey: TransactionTypeCatalog.regular.key, quickUploadId: change.quickUploadId))
        }
    }
}

private struct TypeTitleButton: View {
    let label: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Text(label)
                    .font(.headline)
                Image(systemName: "chevron.down")
                    .font(.caption.weight(.semibold))
                    .accessibilityHidden(true)
            }
        }
        .foregroundStyle(.primary)
        .accessibilityLabel(label)
        .accessibilityHint("Changes the transaction type")
    }
}
