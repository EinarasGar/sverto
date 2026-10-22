import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct AiFeaturesViewModelTests {
    @Test func switchStartsFromTheStoredFlag() {
        let store = AiFeaturesSettingStore(noHandle: .init())
        store.setEnabled(true)

        #expect(AiFeaturesViewModel(store: store).isEnabled)
        #expect(!AiFeaturesViewModel(store: AiFeaturesSettingStore(noHandle: .init())).isEnabled)
    }

    @Test func turningOnShowsTheDisclosureWithoutSaving() {
        let store = AiFeaturesSettingStore(noHandle: .init())
        let model = AiFeaturesViewModel(store: store)

        model.turnOn()

        #expect(model.aiConsent.isPresented)
        #expect(!model.isEnabled)
        #expect(store.requestedValues.isEmpty)
    }

    @Test func consentingSavesAndShowsTheSwitchOn() async {
        let store = AiFeaturesSettingStore(noHandle: .init())
        let model = AiFeaturesViewModel(store: store)
        model.turnOn()

        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()

        #expect(store.requestedValues == [true])
        #expect(model.isEnabled)
    }

    @Test func notNowLeavesTheSwitchOff() {
        let store = AiFeaturesSettingStore(noHandle: .init())
        let model = AiFeaturesViewModel(store: store)
        model.turnOn()

        model.aiConsent.decline()
        model.aiConsent.sheetDismissed()

        #expect(!model.isEnabled)
        #expect(store.requestedValues.isEmpty)
    }

    @Test func turningOffSavesImmediatelyWithoutAsking() async throws {
        let store = AiFeaturesSettingStore(noHandle: .init())
        store.setEnabled(true)
        let model = AiFeaturesViewModel(store: store)

        try await model.turnOff()

        #expect(store.requestedValues == [false])
        #expect(!model.isEnabled)
        #expect(!model.isSaving)
        #expect(!model.aiConsent.isPresented)
    }

    @Test func switchIsBusyWhileTurningOff() async throws {
        let store = AiFeaturesSettingStore(noHandle: .init())
        store.setEnabled(true)
        store.holdSaves()
        let model = AiFeaturesViewModel(store: store)

        let save = Task { try await model.turnOff() }
        try await waitUntil { store.heldSaveCount == 1 }

        #expect(model.isSaving)
        #expect(!model.isEnabled)
        store.releaseSaves()
        try await save.value
        #expect(!model.isSaving)
    }

    @Test func failedTurnOffRevertsTheSwitchAndReportsTheError() async {
        let store = AiFeaturesSettingStore(noHandle: .init())
        store.setEnabled(true)
        store.setFailure(.Network(reason: "Offline"))
        let model = AiFeaturesViewModel(store: store)

        await #expect(throws: ApiError.Network(reason: "Offline")) {
            try await model.turnOff()
        }

        #expect(model.isEnabled)
        #expect(!model.isSaving)
    }

    @Test func refreshPicksUpAFlagChangedElsewhere() {
        let store = AiFeaturesSettingStore(noHandle: .init())
        store.setEnabled(true)
        let model = AiFeaturesViewModel(store: store)

        store.setEnabled(false)
        model.refresh()

        #expect(!model.isEnabled)
    }

    @Test func refreshPicksUpConsentGivenFromAnotherPopup() async {
        let store = AiFeaturesSettingStore(noHandle: .init())
        let model = AiFeaturesViewModel(store: store)
        let elsewhere = AiConsentGate(store: store)
        elsewhere.perform {}
        await elsewhere.turnOn()
        elsewhere.sheetDismissed()

        model.refresh()

        #expect(model.isEnabled)
    }

    private func waitUntil(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition(), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition())
    }
}

nonisolated private final class AiFeaturesSettingStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var enabled = false
    private var failure: ApiError?
    private var requests: [Bool] = []
    private var holdsSaves = false
    private var heldSaves: [CheckedContinuation<Void, Never>] = []

    var requestedValues: [Bool] { lock.withLock { requests } }
    var heldSaveCount: Int { lock.withLock { heldSaves.count } }

    func setEnabled(_ enabled: Bool) {
        lock.withLock { self.enabled = enabled }
    }

    func setFailure(_ failure: ApiError) {
        lock.withLock { self.failure = failure }
    }

    func holdSaves() {
        lock.withLock { holdsSaves = true }
    }

    func releaseSaves() {
        let held = lock.withLock {
            holdsSaves = false
            defer { heldSaves = [] }
            return heldSaves
        }
        held.forEach { $0.resume() }
    }

    override func getAiFeaturesEnabled() -> Bool {
        lock.withLock { enabled }
    }

    override func setAiFeaturesEnabled(enabled: Bool) async throws {
        if lock.withLock({ holdsSaves }) {
            await withCheckedContinuation { continuation in
                lock.withLock { heldSaves.append(continuation) }
            }
        }
        try lock.withLock {
            requests.append(enabled)
            if let failure { throw failure }
            self.enabled = enabled
        }
    }
}
