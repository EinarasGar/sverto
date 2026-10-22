import Foundation
import Observation
import SvertoCore

@Observable
final class AiFeaturesViewModel {
    private(set) var isEnabled: Bool
    private(set) var isSaving = false
    let aiConsent: AiConsentGate

    private let store: AppStore

    init(store: AppStore) {
        self.store = store
        aiConsent = AiConsentGate(store: store)
        isEnabled = store.getAiFeaturesEnabled()
    }

    func refresh() {
        isEnabled = store.getAiFeaturesEnabled()
    }

    func turnOn() {
        aiConsent.requestConsent { [weak self] in self?.refresh() }
    }

    func turnOff() async throws {
        isSaving = true
        isEnabled = false
        defer { isSaving = false }
        do {
            try await store.setAiFeaturesEnabled(enabled: false)
        } catch {
            refresh()
            throw error
        }
    }
}
