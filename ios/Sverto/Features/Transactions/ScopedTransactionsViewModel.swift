import Foundation
import Observation
import SvertoCore

@Observable
final class ScopedTransactionsViewModel {
    nonisolated static let loadMoreBuffer = 5

    private(set) var state = ScopedTransactionsState(
        isLoading: true,
        isLoadingMore: false,
        error: nil,
        items: [],
        hasMore: false
    )
    private(set) var isRefreshing = false

    private let store: AppStore
    private let scope: TransactionListScope
    private var bridge: ScopedTransactionsObserverBridge?
    private var hasLoaded = false

    init(store: AppStore, scope: TransactionListScope) {
        self.store = store
        self.scope = scope
    }

    var showsLoading: Bool { state.isLoading && !isRefreshing }

    var orderedItems: [TransactionListItem] {
        state.items.sorted { $0.date > $1.date }
    }

    var sections: [TransactionDayGroup] {
        TransactionDayGrouping.group(orderedItems)
    }

    func start() async {
        observeIfNeeded()
        guard !hasLoaded else { return }
        hasLoaded = true
        await store.loadScopedTransactions(scope: scope)
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobserveScopedTransactions()
    }

    func refresh() async {
        isRefreshing = true
        await store.refreshScopedTransactions()
        isRefreshing = false
    }

    func loadMoreIfNeeded(appearing transaction: TransactionListItem) async {
        let ordered = orderedItems
        guard let index = ordered.firstIndex(where: { $0.id == transaction.id }),
              index >= ordered.count - Self.loadMoreBuffer,
              state.hasMore, !state.isLoadingMore, !state.isLoading else { return }
        await store.loadMoreScopedTransactions()
    }

    private func observeIfNeeded() {
        guard bridge == nil else { return }
        let bridge = ScopedTransactionsObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.state = state }
        }
        self.bridge = bridge
        store.observeScopedTransactions(observer: bridge)
    }
}
