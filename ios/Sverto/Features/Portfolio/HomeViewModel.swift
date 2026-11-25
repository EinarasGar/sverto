import Foundation
import Observation
import SvertoCore

@Observable
final class HomeViewModel {
    private(set) var state = PortfolioState(
        isLoading: true,
        error: nil,
        holdings: [],
        liabilities: [],
        holdingsTotal: 0,
        liabilitiesTotal: 0,
        chartData: [],
        baseTicker: ""
    )

    private let store: AppStore
    private var bridge: PortfolioObserverBridge?
    private var hasLoaded = false

    init(store: AppStore) {
        self.store = store
    }

    func start() async {
        observeIfNeeded()
        guard !hasLoaded else { return }
        hasLoaded = true
        await store.loadPortfolio()
    }

    func load() async {
        await store.loadPortfolio()
    }

    func refresh() async {
        await store.refreshPortfolio()
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobservePortfolio()
    }

    private func observeIfNeeded() {
        guard bridge == nil else { return }
        let bridge = PortfolioObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in self?.state = state }
        }
        self.bridge = bridge
        store.observePortfolio(observer: bridge)
    }
}
