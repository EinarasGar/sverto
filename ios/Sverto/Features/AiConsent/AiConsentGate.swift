import Foundation
import Observation
import SvertoCore

@Observable
final class AiConsentGate {
    var isPresented = false
    private(set) var isEnabling = false
    private(set) var error: String?

    private let store: AppStore
    private var pendingActions: [() -> Void] = []
    private var isGranted = false

    init(store: AppStore) {
        self.store = store
    }

    func perform(_ action: @escaping () -> Void) {
        if store.getAiFeaturesEnabled() {
            action()
        } else {
            requestConsent(then: action)
        }
    }

    func requestConsent(then action: @escaping () -> Void) {
        pendingActions.append(action)
        error = nil
        isPresented = true
    }

    func turnOn() async {
        guard !isEnabling else { return }
        isEnabling = true
        error = nil
        do {
            try await store.setAiFeaturesEnabled(enabled: true)
            isGranted = true
            isPresented = false
        } catch {
            self.error = error.friendlyMessage
        }
        isEnabling = false
    }

    func decline() {
        isPresented = false
    }

    // Pending actions may present their own UI (photo picker), so they only run once the sheet is fully gone.
    func sheetDismissed() {
        let actions = isGranted ? pendingActions : []
        pendingActions = []
        isGranted = false
        error = nil
        actions.forEach { $0() }
    }
}
