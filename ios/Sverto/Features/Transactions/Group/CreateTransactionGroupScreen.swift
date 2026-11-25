import SvertoCore
import SwiftUI

struct CreateTransactionGroupScreen: View {
    let quickUploadId: String?
    let fromSelection: Bool
    var editGroupId: String?

    @Environment(AppServices.self) private var services

    var body: some View {
        GroupFormView(
            store: services.store,
            quickUploadId: quickUploadId,
            fromSelection: fromSelection,
            editGroupId: editGroupId
        )
    }
}

private struct GroupFormView: View {
    private let quickUploadId: String?
    private let fromSelection: Bool
    private let editGroupId: String?

    @State private var model: CreateTransactionGroupViewModel
    @State private var showTypePicker = false
    @State private var showCategoryPicker = false
    @State private var showsDiscardConfirmation = false
    @State private var pendingAddTypeKey: String?
    @FocusState private var focusedField: FormField?

    @Environment(ShellState.self) private var shell
    @Environment(TransactionsViewModel.self) private var transactions
    @Environment(QuickUploadViewModel.self) private var quickUploads

    init(store: AppStore, quickUploadId: String?, fromSelection: Bool, editGroupId: String?) {
        _model = State(initialValue: CreateTransactionGroupViewModel(store: store))
        self.quickUploadId = quickUploadId
        self.fromSelection = fromSelection
        self.editGroupId = editGroupId
    }

    private var isEditMode: Bool { editGroupId != nil }
    private var isSubmitting: Bool { model.submitState == .submitting }
    private var effectiveQuickUploadId: String? { model.quickUploadId ?? quickUploadId }
    private var childCount: Int { model.formState.transactions.count }

    var body: some View {
        ScrollViewReader { proxy in
            Form {
                if let message = model.loadError {
                    Section {
                        FormErrorRow(message: message)
                        if model.canRetryLoading {
                            Button("Retry") { Task { await model.retryLoading() } }
                        }
                    }
                } else {
                    if let message = model.errorMessage {
                        Section {
                            FormErrorRow(message: message)
                        }
                        .id("group-form-error")
                    }
                    Group {
                        LedgerMatchSection(controller: model.ledgerMatch)
                        GroupDetailsSection(model: model, focus: $focusedField) {
                            model.categories.load()
                            showCategoryPicker = true
                        }
                        transactionsSection
                        if effectiveQuickUploadId != nil {
                            CorrectionInput(state: model.correction.state, focus: $focusedField, onSend: model.sendCorrection)
                        }
                    }
                    .disabled(!model.canEdit)
                }
            }
            .disabled(model.isBusy)
            .scrollDismissesKeyboard(.interactively)
            .navigationTitle(isEditMode ? "Edit Group" : "New Group")
            .navigationBarTitleDisplayMode(.inline)
            .navigationBarBackButtonHidden()
            .toolbar(.hidden, for: .tabBar)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel", action: requestCancel)
                        .tint(.primary)
                        .disabled(model.isBusy)
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
                    .disabled(childCount == 0 || !model.canEdit)
                    .accessibilityLabel(isSubmitting ? "Saving group" : isEditMode ? "Update group" : "Save group")
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
            .sheet(isPresented: $showTypePicker, onDismiss: pushPendingAddRoute) {
                TransactionTypePickerSheet(showGroupOption: false) { typeKey in
                    pendingAddTypeKey = typeKey
                }
            }
            .sheet(isPresented: $showCategoryPicker) {
                CategorySearchScene(
                    results: model.categories.results,
                    isLoading: model.categories.isLoading,
                    onQueryChange: { model.categories.filter($0) },
                    onSelect: { model.selectCategory($0) }
                )
            }
            .task { await start() }
            .onChange(of: shell.pendingGroupTransaction, initial: true) { _, pending in
                consumePendingChild(pending)
            }
            .onChange(of: model.formState.transactions, initial: true) { _, rows in
                shell.groupChildDrafts = rows.map(\.item)
            }
            .onChange(of: model.errorRevision) { _, _ in
                guard let message = model.errorMessage else { return }
                Haptics.error()
                AccessibilityNotification.Announcement(message).post()
                proxy.scrollTo("group-form-error", anchor: .top)
            }
            .onChange(of: model.loadError) { _, message in
                if let message { AccessibilityNotification.Announcement(message).post() }
            }
            .confirmationDialog("Discard changes?", isPresented: $showsDiscardConfirmation, titleVisibility: .visible) {
                Button("Discard Changes", role: .destructive) {
                    guard !model.isBusy else { return }
                    shell.pop()
                }
                Button("Keep Editing", role: .cancel) { }
            } message: {
                Text("Your changes to this group will be lost.")
            }
            .onChange(of: model.submitState) { _, state in
                if state == .success { handleSuccess() }
            }
            .onChange(of: model.correction.typeChange) { _, change in
                guard let change else { return }
                model.correction.consumeTypeChange()
                shell.pop()
                shell.push(.createTransaction(typeKey: TransactionTypeCatalog.regular.key, quickUploadId: change.quickUploadId))
            }
            .sensoryFeedback(.success, trigger: model.submitState == .success)
            .aiConsentSheet(model.correction.aiConsent)
        }
    }

    private func requestCancel() {
        guard !model.isBusy else { return }
        focusedField = nil
        if model.hasUnsavedChanges {
            showsDiscardConfirmation = true
        } else {
            shell.pop()
        }
    }

    private var transactionsSection: some View {
        Section {
            ForEach(Array(model.formState.transactions.enumerated()), id: \.element.id) { index, row in
                GroupTransactionRow(
                    index: index,
                    item: row.item,
                    onEdit: { editChild(at: index) },
                    onRemove: { model.removeTransaction(id: row.id) }
                )
            }
            Button {
                showTypePicker = true
            } label: {
                Label("Add Transaction", systemImage: "plus")
                    .fontWeight(.semibold)
            }
        } header: {
            Text("Transactions (\(childCount))")
        } footer: {
            Text(childCount == 0 ? "Add at least one transaction to save this group." : "\(childCount) transactions")
        }
    }

    private func start() async {
        if let editGroupId {
            guard let group = shell.transactionCache[editGroupId]?.transaction else {
                model.reportMissingGroup()
                return
            }
            await model.initForEdit(group: group)
        } else if let quickUploadId {
            await model.initFromProposal(quickUploadId: quickUploadId)
        } else if fromSelection, !shell.pendingGroupSelection.isEmpty {
            let ids = shell.pendingGroupSelection
            shell.pendingGroupSelection = []
            await model.initFromSelection(transactionIds: ids)
        } else {
            model.initNew()
        }
    }

    private func consumePendingChild(_ pending: GroupTransactionItem?) {
        guard let pending else { return }
        if let index = shell.editingGroupIndex {
            model.updateTransaction(index: index, item: pending)
            shell.editingGroupIndex = nil
        } else {
            model.addTransaction(pending)
        }
        shell.pendingGroupTransaction = nil
    }

    private func pushPendingAddRoute() {
        guard let typeKey = pendingAddTypeKey else { return }
        pendingAddTypeKey = nil
        shell.editingGroupIndex = nil
        shell.push(.groupAddTransaction(typeKey: typeKey))
    }

    private func editChild(at index: Int) {
        let drafts = model.formState.transactions.map(\.item)
        guard drafts.indices.contains(index) else { return }
        shell.groupChildDrafts = drafts
        shell.editingGroupIndex = index
        shell.push(.groupEditTransaction(typeKey: apiTypeToConfigKey(drafts[index].input.typeKey), index: index))
    }

    private func handleSuccess() {
        transactions.clearSelection()
        Task { await transactions.refresh() }
        if let quickUploadId, model.ledgerMatch.mergeTargetId == nil {
            Task { await quickUploads.complete(id: quickUploadId, accepted: true) }
        }
        if let editGroupId {
            shell.transactionCache.removeValue(forKey: editGroupId)
            popPastDetail(of: editGroupId)
        } else {
            shell.pop()
        }
    }

    // The detail screen underneath renders from the cache entry that was just invalidated,
    // so a successful edit returns to the list instead of landing on an empty detail.
    private func popPastDetail(of groupId: String) {
        var path = shell.path(for: shell.selectedTab)
        let detailIsBelow = path.count >= 2 && path[path.count - 2] == .transactionDetail(id: groupId)
        path.removeLast(min(path.count, detailIsBelow ? 2 : 1))
        shell.setPath(path, for: shell.selectedTab)
    }
}
