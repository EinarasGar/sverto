import Foundation
import Observation
import SvertoCore

@Observable
final class ProposalCorrectionController {
    private(set) var state: CorrectionState = .idle
    private(set) var typeChange: CorrectionTypeChange?
    let aiConsent: AiConsentGate

    private let store: AppStore
    private var resetTask: Task<Void, Never>?

    init(store: AppStore) {
        self.store = store
        aiConsent = AiConsentGate(store: store)
    }

    func send<Form>(
        _ message: String,
        uploadId: String,
        expecting expectedType: ProposalType,
        map: @escaping (_ proposalData: String, _ lookupTables: String) -> Form?,
        apply: @escaping (Form) -> Void
    ) {
        aiConsent.perform { [weak self] in
            Task { await self?.submit(message, uploadId: uploadId, expecting: expectedType, map: map, apply: apply) }
        }
    }

    func consumeTypeChange() {
        typeChange = nil
    }

    private func submit<Form>(
        _ message: String,
        uploadId: String,
        expecting expectedType: ProposalType,
        map: @escaping (_ proposalData: String, _ lookupTables: String) -> Form?,
        apply: @escaping (Form) -> Void
    ) async {
        resetTask?.cancel()
        state = .sending
        do {
            let detail = try await store.sendQuickUploadCorrection(uploadId: uploadId, message: message)
            if let returnedType = detail.proposalType.flatMap(ProposalType.init(rawValue:)), returnedType != expectedType {
                state = .idle
                typeChange = CorrectionTypeChange(quickUploadId: uploadId, newProposalType: returnedType)
                return
            }
            guard let corrected = map(detail.proposalData ?? "{}", detail.lookupTables) else {
                state = .idle
                return
            }
            state = .updated
            resetTask = Task {
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                state = .idle
            }
            apply(corrected)
        } catch ApiError.AiFeaturesDisabled {
            state = .idle
            aiConsent.requestConsent { [weak self] in
                self?.send(message, uploadId: uploadId, expecting: expectedType, map: map, apply: apply)
            }
        } catch {
            state = .idle
        }
    }
}
