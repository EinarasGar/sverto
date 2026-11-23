import PhotosUI
import SwiftUI
import SvertoCore

struct TransactionsScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        TransactionsTab(store: services.store)
    }
}

private struct TransactionsTab: View {
    @State private var search: TransactionSearchViewModel
    @State private var held: HeldTransactionsViewModel
    @State private var searchText = ""
    @State private var isSearchPresented = false
    @Environment(TransactionsViewModel.self) private var model
    @Environment(QuickUploadViewModel.self) private var quickUploads
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    init(store: AppStore) {
        _search = State(initialValue: TransactionSearchViewModel(store: store))
        _held = State(initialValue: HeldTransactionsViewModel(store: store))
    }

    var body: some View {
        content
            .navigationTitle("Transactions")
            .navigationBarTitleDisplayMode(model.selectionActive ? .inline : .large)
            .modifier(TransactionSearchToolbar(
                isEnabled: !model.selectionActive,
                text: $searchText,
                isPresented: $isSearchPresented
            ))
            .onChange(of: searchText) { _, query in search.onQueryChange(query) }
            .onChange(of: isSearchPresented) { _, presented in
                if presented { model.clearSelection() }
            }
            .modifier(NewTransactionEntryPoints(
                quickUploads: quickUploads,
                isHidden: model.selectionActive,
                isSearchPresented: $isSearchPresented
            ))
            .modifier(SelectionActions(model: model))
            .onChange(of: model.actionFailure) { _, failure in
                if let failure { toasts.show(failure.message) }
            }
            .onChange(of: held.notice) { _, notice in
                if let notice { toasts.show(notice.message) }
            }
            .sensoryFeedback(.success, trigger: held.decidedCount)
            .onChange(of: shell.pendingSharedImages, initial: true) { _, images in
                guard !images.isEmpty else { return }
                shell.pendingSharedImages = []
                quickUploads.aiConsent.perform {
                    Task { await queueSharedImages(images) }
                }
            }
            .aiConsentSheet(quickUploads.aiConsent)
            .onAppear { model.start() }
            .onDisappear { model.stop() }
            .task { await held.load() }
    }

    @ViewBuilder
    private var content: some View {
        if isSearchPresented {
            TransactionSearchResults(model: search) { transaction in
                shell.showTransactionDetail(transaction, isInGroup: false)
            }
        } else if model.state.isLoading, model.state.items.isEmpty {
            TransactionListSkeleton()
        } else if let error = model.state.error, model.state.items.isEmpty {
            TransactionListErrorState(message: error) {
                Task { await model.load() }
            }
        } else {
            TransactionListView(
                model: model,
                quickUploads: quickUploads,
                held: held,
                onTransactionTap: { shell.showTransactionDetail($0, isInGroup: false) },
                onQuickUploadTap: openQuickUpload
            )
        }
    }

    private func openQuickUpload(_ item: QuickUploadUiItem) {
        switch item.proposalType {
        case .transactionGroup:
            shell.push(.createTransactionGroup(quickUploadId: item.id, fromSelection: false))
        case .transaction, nil:
            shell.push(.createTransaction(typeKey: TransactionTypeCatalog.regular.key, quickUploadId: item.id))
        }
    }

    private func queueSharedImages(_ images: [Data]) async {
        for imageData in images {
            if let prepared = await ReceiptPhotoImport.prepare(imageData: imageData, mimeType: nil) {
                quickUploads.queue(prepared)
            } else {
                toasts.show(ReceiptPhotoImport.failureMessage)
            }
        }
    }
}

private struct TransactionSearchToolbar: ViewModifier {
    let isEnabled: Bool
    @Binding var text: String
    @Binding var isPresented: Bool

    @ViewBuilder
    func body(content: Content) -> some View {
        if isEnabled {
            content.modifier(ToolbarSearch(text: $text, isPresented: $isPresented, prompt: "Search transactions"))
        } else {
            content
        }
    }
}

private struct NewTransactionEntryPoints: ViewModifier {
    let quickUploads: QuickUploadViewModel
    let isHidden: Bool
    @Binding var isSearchPresented: Bool

    @State private var isTypePickerPresented = false
    @State private var isPhotoPickerPresented = false
    @State private var pickedPhoto: PhotosPickerItem?
    @State private var routeAfterSheetDismiss: AppRoute?
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    func body(content: Content) -> some View {
        content
            .toolbar {
                if !isHidden && !isSearchPresented {
                    ToolbarItemGroup(placement: .topBarTrailing) {
                        Button("Search transactions", systemImage: "magnifyingglass") {
                            isSearchPresented = true
                        }
                        .labelStyle(.iconOnly)
                        .tint(.primary)
                        NewTransactionMenu(
                            onAddReceiptPhoto: { quickUploads.aiConsent.perform { isPhotoPickerPresented = true } },
                            onManualEntry: { isTypePickerPresented = true },
                            onNewGroup: { shell.push(.createTransactionGroup(quickUploadId: nil, fromSelection: false)) },
                            onImportStatements: { shell.push(.imports) }
                        )
                    }
                    ProfileToolbar()
                }
            }
            .sheet(isPresented: $isTypePickerPresented, onDismiss: pushRouteAfterSheetDismiss) {
                TransactionTypePickerSheet(
                    onSelectType: { routeAfterSheetDismiss = .createTransaction(typeKey: $0, quickUploadId: nil) },
                    onSelectGroup: { routeAfterSheetDismiss = .createTransactionGroup(quickUploadId: nil, fromSelection: false) }
                )
            }
            .photosPicker(isPresented: $isPhotoPickerPresented, selection: $pickedPhoto, matching: .images)
            .onChange(of: pickedPhoto) { _, item in
                guard let item else { return }
                pickedPhoto = nil
                Task { await importReceipt(item) }
            }
    }

    private func pushRouteAfterSheetDismiss() {
        guard let route = routeAfterSheetDismiss else { return }
        routeAfterSheetDismiss = nil
        shell.push(route)
    }

    private func importReceipt(_ item: PhotosPickerItem) async {
        guard let prepared = await ReceiptPhotoImport.prepare(item) else {
            toasts.show(ReceiptPhotoImport.failureMessage)
            return
        }
        quickUploads.queue(prepared)
    }
}

private struct SelectionActions: ViewModifier {
    let model: TransactionsViewModel

    @State private var isDeleteConfirmationPresented = false
    @Environment(ShellState.self) private var shell

    func body(content: Content) -> some View {
        content
            .toolbar(model.selectionActive ? .hidden : .automatic, for: .tabBar)
            .toolbar {
                if model.selectionActive {
                    SelectionToolbar(
                        count: model.selectedIds.count,
                        isBusy: model.isBulkBusy,
                        showMarkReviewed: model.hasGhostSelected,
                        showGroup: model.canGroupSelection,
                        onClose: model.clearSelection,
                        onMarkReviewed: { Task { await model.markSelectedReviewed() } },
                        onGroup: groupSelection,
                        onDelete: { isDeleteConfirmationPresented = true }
                    )
                }
            }
            .sensoryFeedback(.selection, trigger: model.selectedIds)
            .confirmationDialog("Delete selection?", isPresented: $isDeleteConfirmationPresented, titleVisibility: .visible) {
                Button("Delete", role: .destructive) {
                    Task { await model.deleteSelected() }
                }
            } message: {
                Text(model.deleteSelectionMessage)
            }
    }

    private func groupSelection() {
        shell.pendingGroupSelection = model.selectedItems.map(\.id)
        shell.push(.createTransactionGroup(quickUploadId: nil, fromSelection: true))
    }
}
