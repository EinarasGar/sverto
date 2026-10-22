import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct QuickUploadConsentTests {
    private let prepared = PreparedQuickUpload(imageData: Data([1, 2, 3]), thumbnail: Data(), mimeType: "image/jpeg")

    @Test func retryWhileDisabledAsksBeforeRefreshing() async throws {
        let store = QuickUploadConsentStore(noHandle: .init())
        let model = QuickUploadViewModel(store: store)

        model.retry(id: "upload")

        #expect(model.aiConsent.isPresented)
        #expect(store.refreshCount == 0)
        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()
        try await waitUntil { store.refreshCount == 1 }
    }

    @Test func uploadRejectedByTheServerAsksAndFlushesTheHeldUploadAfterConsent() async throws {
        let store = QuickUploadConsentStore(noHandle: .init())
        store.setEnabled(true)
        store.rejectNextFlush()
        let model = QuickUploadViewModel(store: store)

        model.queue(prepared)
        try await waitUntil { model.aiConsent.isPresented }

        #expect(store.refreshCount == 1)
        await model.aiConsent.turnOn()
        model.aiConsent.sheetDismissed()
        try await waitUntil { store.refreshCount == 2 }
        #expect(store.queuedImages == [prepared.imageData])
        #expect(!model.aiConsent.isPresented)
    }

    @Test func rejectedCorrectionAsksAndResendsAfterConsent() async throws {
        let store = QuickUploadConsentStore(noHandle: .init())
        store.setEnabled(true)
        store.rejectNextCorrection()
        let controller = ProposalCorrectionController(store: store)
        var applied: [String] = []

        controller.send("the date is March 7", uploadId: "upload", expecting: .transaction) { proposalData, _ in
            proposalData
        } apply: { applied.append($0) }
        try await waitUntil { controller.aiConsent.isPresented }

        #expect(controller.state == .idle)
        #expect(applied.isEmpty)
        await controller.aiConsent.turnOn()
        controller.aiConsent.sheetDismissed()
        try await waitUntil { applied == ["corrected"] }
        #expect(store.correctionMessages == ["the date is March 7", "the date is March 7"])
    }

    @Test func correctionWhileDisabledAsksBeforeSending() {
        let store = QuickUploadConsentStore(noHandle: .init())
        let controller = ProposalCorrectionController(store: store)

        controller.send("the date is March 7", uploadId: "upload", expecting: .transaction) { proposalData, _ in
            proposalData
        } apply: { _ in }

        #expect(controller.aiConsent.isPresented)
        #expect(controller.state == .idle)
        #expect(store.correctionMessages.isEmpty)
    }

    private func waitUntil(_ condition: () -> Bool) async throws {
        let deadline = ContinuousClock.now + .seconds(2)
        while !condition(), ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition())
    }
}

nonisolated private final class QuickUploadConsentStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var enabled = false
    private var rejectsFlush = false
    private var rejectsCorrection = false
    private var queued: [Data] = []
    private var refreshes = 0
    private var corrections: [String] = []

    var queuedImages: [Data] { lock.withLock { queued } }
    var refreshCount: Int { lock.withLock { refreshes } }
    var correctionMessages: [String] { lock.withLock { corrections } }

    func setEnabled(_ enabled: Bool) {
        lock.withLock { self.enabled = enabled }
    }

    func rejectNextFlush() {
        lock.withLock { rejectsFlush = true }
    }

    func rejectNextCorrection() {
        lock.withLock { rejectsCorrection = true }
    }

    override func getAiFeaturesEnabled() -> Bool {
        lock.withLock { enabled }
    }

    override func setAiFeaturesEnabled(enabled: Bool) async throws {
        lock.withLock { self.enabled = enabled }
    }

    override func queueQuickUpload(imageData: Data, thumbnail: Data, mimeType: String) {
        lock.withLock { queued.append(imageData) }
    }

    override func refreshQuickUploads() async {
        lock.withLock {
            refreshes += 1
            if rejectsFlush {
                rejectsFlush = false
                enabled = false
            }
        }
    }

    override func sendQuickUploadCorrection(uploadId: String, message: String) async throws -> QuickUploadDetail {
        try lock.withLock {
            corrections.append(message)
            if rejectsCorrection {
                rejectsCorrection = false
                throw ApiError.AiFeaturesDisabled
            }
        }
        return QuickUploadDetail(status: "proposal_ready", sourceFileId: "file", proposalType: "transaction", proposalData: "corrected", createdAt: 0, updatedAt: 0, lookupTables: "{}")
    }
}
