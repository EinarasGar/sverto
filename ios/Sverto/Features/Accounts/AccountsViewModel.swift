import Foundation
import Observation
import SvertoCore

struct AccountTypeSection: Identifiable {
    let typeId: Int32
    let accounts: [AccountListItem]

    var id: Int32 { typeId }
}

@Observable
final class AccountsViewModel {
    private(set) var state = AccountsState(
        isLoading: true,
        isLoadingBalances: false,
        error: nil,
        accounts: [],
        totalNetWorth: 0,
        baseTicker: ""
    )
    private(set) var isRefreshing = false

    private let store: AppStore
    private var bridge: AccountsObserverBridge?
    private var hasLoaded = false
    private var visibleScreens = 0

    init(store: AppStore) {
        self.store = store
    }

    var showsSkeleton: Bool {
        state.isLoading && !isRefreshing && state.accounts.isEmpty
    }

    var sections: [AccountTypeSection] {
        let listed = state.accounts.filter { $0.linkedPropertyId == nil }
        let grouped = Dictionary(grouping: listed, by: \.accountTypeId)
        return AccountTypeVisuals.orderedTypeIds(grouped.keys).compactMap { typeId in
            grouped[typeId].map { AccountTypeSection(typeId: typeId, accounts: $0) }
        }
    }

    // The tab root is briefly on screen twice while it moves between the stack and the list column,
    // so the observer stops only when the last copy disappears.
    func start() async {
        visibleScreens += 1
        observeIfNeeded()
        guard !hasLoaded else { return }
        hasLoaded = true
        await store.loadAccounts()
    }

    func stop() {
        visibleScreens = max(0, visibleScreens - 1)
        guard visibleScreens == 0, bridge != nil else { return }
        bridge = nil
        store.unobserveAccounts()
    }

    func retry() async {
        await store.loadAccounts()
    }

    func refresh() async {
        isRefreshing = true
        await store.refreshAccounts()
        isRefreshing = false
    }

    private func observeIfNeeded() {
        guard bridge == nil else { return }
        let bridge = AccountsObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.state = state }
        }
        self.bridge = bridge
        store.observeAccounts(observer: bridge)
    }
}
