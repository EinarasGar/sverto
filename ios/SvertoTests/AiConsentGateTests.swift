import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct AiConsentGateTests {
    @Test func enabledFeaturesRunTheActionWithoutAsking() {
        let store = AiFeaturesStore(noHandle: .init())
        store.setEnabled(true)
        let gate = AiConsentGate(store: store)
        var runs = 0

        gate.perform { runs += 1 }

        #expect(runs == 1)
        #expect(!gate.isPresented)
    }

    @Test func disabledFeaturesAskBeforeRunningTheAction() {
        let gate = AiConsentGate(store: AiFeaturesStore(noHandle: .init()))
        var runs = 0

        gate.perform { runs += 1 }

        #expect(runs == 0)
        #expect(gate.isPresented)
    }

    @Test func turningOnEnablesFeaturesAndContinuesOnceTheSheetIsGone() async {
        let store = AiFeaturesStore(noHandle: .init())
        let gate = AiConsentGate(store: store)
        var runs = 0
        gate.perform { runs += 1 }

        await gate.turnOn()

        #expect(store.requestedValues == [true])
        #expect(!gate.isPresented)
        #expect(!gate.isEnabling)
        #expect(runs == 0)
        gate.sheetDismissed()
        #expect(runs == 1)
        gate.perform { runs += 1 }
        #expect(runs == 2)
        #expect(!gate.isPresented)
    }

    @Test func failedTurnOnKeepsTheSheetOpenWithoutContinuing() async {
        let store = AiFeaturesStore(noHandle: .init())
        store.setFailure(.Network(reason: "Offline"))
        let gate = AiConsentGate(store: store)
        var runs = 0
        gate.perform { runs += 1 }

        await gate.turnOn()

        #expect(gate.isPresented)
        #expect(gate.error == "Offline")
        #expect(!gate.isEnabling)
        #expect(runs == 0)
    }

    @Test func successfulRetryAfterAFailedTurnOnClearsTheErrorAndContinues() async {
        let store = AiFeaturesStore(noHandle: .init())
        store.setFailure(.Network(reason: "Offline"))
        let gate = AiConsentGate(store: store)
        var runs = 0
        gate.perform { runs += 1 }
        await gate.turnOn()
        store.setFailure(nil)

        await gate.turnOn()
        gate.sheetDismissed()

        #expect(gate.error == nil)
        #expect(store.requestedValues == [true, true])
        #expect(runs == 1)
    }

    @Test func notNowDropsTheActionAndTheNextAttemptAsksAgain() {
        let store = AiFeaturesStore(noHandle: .init())
        let gate = AiConsentGate(store: store)
        var runs = 0
        gate.perform { runs += 1 }

        gate.decline()
        gate.sheetDismissed()

        #expect(!gate.isPresented)
        #expect(runs == 0)
        #expect(store.requestedValues.isEmpty)
        gate.perform { runs += 1 }
        #expect(gate.isPresented)
        #expect(runs == 0)
    }

    @Test func serverRejectionAsksEvenWhenTheCachedFlagIsOn() {
        let store = AiFeaturesStore(noHandle: .init())
        store.setEnabled(true)
        let gate = AiConsentGate(store: store)
        var runs = 0

        gate.requestConsent { runs += 1 }

        #expect(gate.isPresented)
        #expect(runs == 0)
    }

    @Test func everyActionWaitingOnConsentRunsInOrderOnceTurnedOn() async {
        let gate = AiConsentGate(store: AiFeaturesStore(noHandle: .init()))
        var runs: [String] = []
        gate.perform { runs.append("first") }
        gate.requestConsent { runs.append("second") }

        await gate.turnOn()
        gate.sheetDismissed()

        #expect(runs == ["first", "second"])
    }
}

nonisolated private final class AiFeaturesStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var enabled = false
    private var failure: ApiError?
    private var requests: [Bool] = []

    var requestedValues: [Bool] { lock.withLock { requests } }

    func setEnabled(_ enabled: Bool) {
        lock.withLock { self.enabled = enabled }
    }

    func setFailure(_ failure: ApiError?) {
        lock.withLock { self.failure = failure }
    }

    override func getAiFeaturesEnabled() -> Bool {
        lock.withLock { enabled }
    }

    override func setAiFeaturesEnabled(enabled: Bool) async throws {
        try lock.withLock {
            requests.append(enabled)
            if let failure { throw failure }
            self.enabled = enabled
        }
    }
}
