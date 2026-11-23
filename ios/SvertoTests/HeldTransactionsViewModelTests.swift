import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct HeldTransactionsViewModelTests {
    @Test func decidingSendsTheChosenCandidateAndAConflictReloadsWithANeutralNote() async {
        let store = HeldStore(noHandle: .init())
        store.listed = [Self.held("h1"), Self.held("h2")]
        let model = HeldTransactionsViewModel(store: store)
        await model.load()

        store.listed = [Self.held("h2")]
        await model.decide(Self.held("h1"), decision: .same(transactionId: "t1"))

        #expect(store.decisions == [HeldDecisionCall(heldTransactionId: "h1", decision: .same(transactionId: "t1"))])
        #expect(model.items.map(\.id) == ["h2"])
        #expect(model.notice == nil)

        store.conflict = true
        store.listed = [Self.held("h3")]
        await model.decide(Self.held("h2"), decision: .different)

        #expect(store.decisions.last == HeldDecisionCall(heldTransactionId: "h2", decision: .different))
        #expect(model.items.map(\.id) == ["h3"])
        #expect(model.notice != nil)
        #expect(model.decidingId == nil)
    }

    private static func held(_ id: String) -> HeldTransaction {
        HeldTransaction(id: id, providerKind: "enablebanking", date: 0, description: "Coffee \(id)", accountId: "a", accountName: "Current", amountDisplay: "-€3.50", candidates: [])
    }
}

nonisolated private struct HeldDecisionCall: Equatable {
    let heldTransactionId: String
    let decision: HeldTransactionDecision
}

nonisolated private final class HeldStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var rows: [HeldTransaction] = []
    private var conflicts = false
    private var recorded: [HeldDecisionCall] = []

    var listed: [HeldTransaction] {
        get { lock.withLock { rows } }
        set { lock.withLock { rows = newValue } }
    }

    var conflict: Bool {
        get { lock.withLock { conflicts } }
        set { lock.withLock { conflicts = newValue } }
    }

    var decisions: [HeldDecisionCall] { lock.withLock { recorded } }

    override func listHeldTransactions() async throws -> [HeldTransaction] { listed }

    override func decideHeldTransaction(heldTransactionId: String, decision: HeldTransactionDecision) async throws {
        lock.withLock { recorded.append(HeldDecisionCall(heldTransactionId: heldTransactionId, decision: decision)) }
        if conflict {
            throw ApiError.Server(reason: "Conflict", status: 409)
        }
    }

    override func refreshTransactions() async {}
}
