import Foundation
import Observation
import SvertoCore

struct HeldTransactionsNotice: Equatable {
    let id = UUID()
    let message: String
}

@Observable
final class HeldTransactionsViewModel {
    private(set) var items: [HeldTransaction] = []
    private(set) var decidingId: String?
    private(set) var decidedCount = 0
    private(set) var notice: HeldTransactionsNotice?

    private let store: AppStore

    init(store: AppStore) {
        self.store = store
    }

    func load() async {
        guard let items = try? await store.listHeldTransactions() else { return }
        self.items = items
    }

    func decide(_ held: HeldTransaction, decision: HeldTransactionDecision) async {
        guard decidingId == nil else { return }
        decidingId = held.id
        do {
            try await store.decideHeldTransaction(heldTransactionId: held.id, decision: decision)
            items.removeAll { $0.id == held.id }
            decidedCount += 1
            await store.refreshTransactions()
        } catch ApiError.Server(_, 409) {
            notice = HeldTransactionsNotice(
                message: "“\(held.description)” changed since this list loaded. It now shows what is left to decide."
            )
        } catch {
            notice = HeldTransactionsNotice(message: error.friendlyMessage)
        }
        decidingId = nil
        await load()
    }
}
