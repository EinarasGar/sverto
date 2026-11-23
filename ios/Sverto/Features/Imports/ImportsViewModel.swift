import Foundation
import Observation
import SvertoCore

@Observable
final class ImportsViewModel {
    private(set) var sessions: [ImportSessionSummary]?
    private(set) var loadError: String?
    private(set) var creating = false
    private(set) var toast: ImportToast?
    let aiConsent: AiConsentGate

    private let store: AppStore

    init(store: AppStore) {
        self.store = store
        aiConsent = AiConsentGate(store: store)
    }

    var inProgress: [ImportSessionSummary] {
        (sessions ?? []).filter { $0.status != .completed }
    }

    var finished: [ImportSessionSummary] {
        (sessions ?? []).filter { $0.status == .completed }
    }

    func load() async {
        do {
            sessions = try await store.listImportSessions().sorted { $0.createdAt > $1.createdAt }
            loadError = nil
        } catch {
            if sessions == nil {
                loadError = error.friendlyMessage
            } else {
                toast = ImportToast(text: error.friendlyMessage)
            }
        }
    }

    func create(onCreated: @escaping (String) -> Void) async {
        guard !creating else { return }
        creating = true
        defer { creating = false }
        do {
            onCreated(try await store.createImportSession())
        } catch ApiError.AiFeaturesDisabled {
            aiConsent.requestConsent { [weak self] in
                Task { await self?.create(onCreated: onCreated) }
            }
        } catch {
            toast = ImportToast(text: error.friendlyMessage)
        }
    }
}
