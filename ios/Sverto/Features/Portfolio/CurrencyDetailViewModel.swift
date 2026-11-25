import Foundation
import Observation
import SvertoCore

@Observable
final class CurrencyDetailViewModel {
    private(set) var detail: CurrencyDetail?
    private(set) var loadError: String?

    private let store: AppStore
    private let assetId: Int32
    private let accountId: String?

    init(store: AppStore, assetId: Int32, accountId: String?) {
        self.store = store
        self.assetId = assetId
        self.accountId = accountId
    }

    func load() async {
        await fetch(refresh: false)
    }

    func refresh() async {
        await fetch(refresh: true)
    }

    private func fetch(refresh: Bool) async {
        do {
            detail = try await store.getCurrencyDetail(assetId: assetId, accountId: accountId, refresh: refresh)
            loadError = nil
        } catch {
            loadError = error.friendlyMessage
        }
    }
}
