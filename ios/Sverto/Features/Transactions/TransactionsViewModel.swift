import Foundation
import Observation
import SvertoCore

struct TransactionActionFailure: Equatable {
    let id = UUID()
    let message: String
}

@Observable
final class TransactionsViewModel {
    private(set) var state = TransactionsState(isLoading: true, isLoadingMore: false, error: nil, items: [], hasMore: false)
    private(set) var selectedIds: Set<String> = []
    private(set) var isBulkBusy = false
    private(set) var actionFailure: TransactionActionFailure?

    private let store: AppStore
    private var bridge: TransactionsObserverBridge?
    private var hasLoaded = false
    private var visibleScreens = 0

    init(store: AppStore) {
        self.store = store
    }

    var selectionActive: Bool { !selectedIds.isEmpty }

    var selectedItems: [TransactionListItem] {
        state.items.filter { selectedIds.contains($0.id) }
    }

    var hasGhostSelected: Bool {
        selectedItems.contains { item in
            item.visibility == .ghost || item.children.contains { $0.visibility == .ghost }
        }
    }

    var selectedGroupCount: Int {
        selectedItems.filter(\.isGroup).count
    }

    var canGroupSelection: Bool {
        selectedIds.count >= 2 && selectedGroupCount == 0
    }

    var deleteSelectionMessage: String {
        Self.deleteConfirmationMessage(transactionCount: selectedIds.count - selectedGroupCount, groupCount: selectedGroupCount)
    }

    // The tab root is briefly on screen twice while it moves between the stack and the list column,
    // so the observer stops only when the last copy disappears.
    func start() {
        visibleScreens += 1
        guard bridge == nil else { return }
        let bridge = TransactionsObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.apply(state) }
        }
        self.bridge = bridge
        store.observeTransactions(observer: bridge)
        if !hasLoaded {
            hasLoaded = true
            Task { await load() }
        }
    }

    func stop() {
        visibleScreens = max(0, visibleScreens - 1)
        guard visibleScreens == 0, bridge != nil else { return }
        bridge = nil
        store.unobserveTransactions()
    }

    func load() async {
        await store.loadTransactions()
    }

    func loadMore() async {
        await store.loadMoreTransactions()
    }

    func refresh() async {
        await store.refreshTransactions()
    }

    func deleteTransaction(id: String) async -> Bool {
        do {
            try await store.deleteTransaction(txId: id)
            return true
        } catch {
            report(error)
            return false
        }
    }

    func deleteTransactionGroup(id: String) async -> Bool {
        do {
            try await store.deleteTransactionGroup(groupId: id)
            return true
        } catch {
            report(error)
            return false
        }
    }

    func markReviewed(id: String) async {
        do {
            try await store.setTransactionVisibility(transactionId: id, visibility: .default)
            await refresh()
        } catch {
            report(error)
        }
    }

    func toggleSelection(_ transactionId: String) {
        if !selectedIds.insert(transactionId).inserted {
            selectedIds.remove(transactionId)
        }
    }

    func clearSelection() {
        selectedIds.removeAll()
    }

    func markSelectedReviewed() async {
        guard !isBulkBusy else { return }
        let transactionIds = selectedItems.flatMap { item in
            item.isGroup ? item.children.map(\.id) : [item.id]
        }
        guard !transactionIds.isEmpty else { return }
        isBulkBusy = true
        defer { isBulkBusy = false }
        do {
            try await store.setTransactionsVisibility(transactionIds: transactionIds, visibility: .default)
            clearSelection()
            await refresh()
        } catch {
            report(error)
        }
    }

    func deleteSelected() async {
        guard !isBulkBusy else { return }
        let items = selectedItems
        guard !items.isEmpty else { return }
        let groupIds = items.filter(\.isGroup).map(\.id)
        let transactionIds = items.filter { !$0.isGroup }.map(\.id)
        isBulkBusy = true
        defer { isBulkBusy = false }
        do {
            try await store.deleteTransactions(transactionIds: transactionIds, groupIds: groupIds)
            clearSelection()
        } catch {
            report(error)
        }
    }

    private func apply(_ newState: TransactionsState) {
        state = newState
        if !selectedIds.isEmpty, !newState.isLoading {
            selectedIds = selectedIds.intersection(newState.items.map(\.id))
        }
    }

    private func report(_ error: Error) {
        actionFailure = TransactionActionFailure(message: error.friendlyMessage)
    }

    nonisolated static func deleteConfirmationMessage(transactionCount: Int, groupCount: Int) -> String {
        let transactions = "\(transactionCount) transaction\(transactionCount == 1 ? "" : "s")"
        let groups = "\(groupCount) group\(groupCount == 1 ? "" : "s")"
        let groupWarning = "Deleting a group also deletes the transactions inside it."
        if groupCount == 0 {
            return "Delete \(transactions)? This action can't be undone."
        }
        if transactionCount == 0 {
            return "Delete \(groups)? \(groupWarning)"
        }
        return "Delete \(transactions) and \(groups)? \(groupWarning)"
    }
}
