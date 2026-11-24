import Foundation
import Observation
import SvertoCore

@Observable
final class LedgerMatchController {
    private(set) var matches: [TransactionCandidate] = []
    private(set) var selectedId: String?
    var mergesIntoMatch = true

    var mergeTargetId: String? { mergesIntoMatch ? selectedId : nil }

    private let store: AppStore

    init(store: AppStore) {
        self.store = store
    }

    func load(uploadId: String) async {
        matches = (try? await store.getQuickUploadMatches(uploadId: uploadId)) ?? []
        if !matches.contains(where: { $0.id == selectedId }) {
            selectedId = matches.first?.id
        }
    }

    func select(_ id: String) {
        selectedId = id
        mergesIntoMatch = true
    }
}
