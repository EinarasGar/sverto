import Foundation
import Observation
import SvertoCore

@Observable
final class ImportItemsViewModel {
    private(set) var items: ImportItems?
    private(set) var loadError: String?
    private(set) var toast: ImportToast?

    private let store: AppStore
    private let sessionId: String
    private var generation = 0

    init(store: AppStore, sessionId: String) {
        self.store = store
        self.sessionId = sessionId
    }

    func load() async {
        do {
            try await fetch(startedAt: generation)
        } catch {
            if items == nil {
                loadError = error.friendlyMessage
            } else {
                toast = ImportToast(text: error.friendlyMessage)
            }
        }
    }

    private func fetch(startedAt: Int) async throws {
        let items = try await store.listImportItems(sessionId: sessionId)
        guard generation == startedAt else { return }
        self.items = items
        loadError = nil
    }

    private func write(_ block: () async throws -> Void) async {
        generation += 1
        do {
            try await block()
        } catch {
            toast = ImportToast(text: error.friendlyMessage)
        }
        generation += 1
        do {
            try await fetch(startedAt: generation)
        } catch {
            toast = ImportToast(text: error.friendlyMessage)
        }
    }

    func setVerb(itemId: String, verb: ImportItemVerb) async {
        await write { try await store.setImportItemsVerb(sessionId: sessionId, itemIds: [itemId], verb: verb, candidateId: nil) }
    }

    func keep(askKey: String) async {
        await write { try await store.answerImportAsk(sessionId: sessionId, askKey: askKey, answer: .keepUnlisted) }
    }
}
