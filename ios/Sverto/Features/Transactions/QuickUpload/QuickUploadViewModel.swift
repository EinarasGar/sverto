import Foundation
import Observation
import SvertoCore

@Observable
final class QuickUploadViewModel {
    private(set) var items: [QuickUploadUiItem] = []
    let aiConsent: AiConsentGate

    private let store: AppStore
    private var bridge: QuickUploadsObserverBridge?

    init(store: AppStore) {
        self.store = store
        aiConsent = AiConsentGate(store: store)
    }

    func start() {
        guard bridge == nil else { return }
        let bridge = QuickUploadsObserverBridge { [weak self] state in
            Task { @MainActor [weak self] in
                self?.items = state.items.map(QuickUploadUiItem.init(item:))
            }
        }
        self.bridge = bridge
        store.observeQuickUploads(observer: bridge)
        Task { await store.refreshQuickUploads() }
    }

    func stop() {
        guard bridge != nil else { return }
        bridge = nil
        store.unobserveQuickUploads()
    }

    func queue(_ prepared: PreparedQuickUpload) {
        let store = self.store
        Task {
            await Task.detached {
                store.queueQuickUpload(data: prepared.data, thumbnail: prepared.thumbnail, mimeType: prepared.mimeType)
            }.value
            await store.refreshQuickUploads()
            // The flush reports no error; the core signals an AI-disabled rejection only by clearing this flag
            // and holds the upload in its queue until AI features are turned back on.
            if !store.getAiFeaturesEnabled() {
                aiConsent.requestConsent {
                    Task { await store.refreshQuickUploads() }
                }
            }
        }
    }

    func refresh() async {
        await store.refreshQuickUploads()
    }

    func retry(id: String) {
        let store = self.store
        aiConsent.perform { [weak self] in
            Task { [weak self] in
                do {
                    try await store.retryQuickUpload(id: id)
                } catch ApiError.AiFeaturesDisabled {
                    self?.retry(id: id)
                } catch {
                    guard let self else { return }
                    items = items.map { item in
                        guard item.id == id else { return item }
                        return QuickUploadUiItem(
                            id: item.id,
                            status: item.status,
                            thumbnail: item.thumbnail,
                            proposalType: item.proposalType,
                            proposalSummary: item.proposalSummary,
                            errorMessage: error.friendlyMessage,
                            createdAt: item.createdAt
                        )
                    }
                }
            }
        }
    }

    func dismiss(id: String) async {
        items.removeAll { $0.id == id }
        await store.dismissQuickUpload(id: id)
    }

    func complete(id: String, accepted: Bool) async {
        items.removeAll { $0.id == id }
        await store.completeQuickUpload(uploadId: id, accepted: accepted)
    }
}

private extension QuickUploadUiItem {
    init(item: UnifiedQuickUploadItem) {
        self.init(
            id: item.id,
            status: QuickUploadStatus(serverStatus: item.status),
            thumbnail: item.thumbnail,
            proposalType: item.proposalType.flatMap(ProposalType.init(rawValue:)),
            proposalSummary: ProposalMapper.summary(proposalData: item.proposalData),
            errorMessage: item.errorMessage,
            createdAt: item.createdAt
        )
    }
}
