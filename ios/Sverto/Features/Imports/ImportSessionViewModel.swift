import Foundation
import Observation
import SvertoCore

@Observable
final class ImportSessionViewModel {
    static let newTransactionChoice = "new"

    private(set) var detail: ImportSessionDetail?
    private(set) var items: ImportItems?
    private(set) var pending: [ImportPendingRow] = []
    private(set) var accounts: [AccountItem]?
    private(set) var accountChoices: [String: String] = [:]
    private(set) var separateAccounts: Set<String> = []
    private(set) var assetChoices: [String: ImportAssetCandidate] = [:]
    private(set) var duplicateChoices: [String: String] = [:]
    private(set) var seenDuplicates: Set<String> = []
    private(set) var applied: AppliedCounts?
    private(set) var uploads: [LocalUpload] = []
    private(set) var loadError: String?
    private(set) var toast: ImportToast?
    private(set) var writes = 0
    private(set) var applying = false
    let aiConsent: AiConsentGate

    private let store: AppStore
    private let sessionId: String
    private var resumed = false
    private var pollTask: Task<Void, Never>?
    private var generation = 0
    private var pickedFiles: [String: PickedFile] = [:]
    private var createAccountFor: String?
    private var accountIdsBeforeCreate: Set<String> = []

    private static let pollInterval: Duration = .seconds(3)

    init(store: AppStore, sessionId: String) {
        self.store = store
        self.sessionId = sessionId
        aiConsent = AiConsentGate(store: store)
    }

    var working: Bool { writes > 0 || applying }

    var readyItems: ImportItems? {
        items?.ready == true ? items : nil
    }

    var blocks: [SessionBlock] {
        guard let detail else { return [] }
        return ImportSessionBlocks.blocks(detail: detail, items: items, seenDuplicates: seenDuplicates, applied: applied)
    }

    var applyBar: ApplyBarState? {
        guard let detail, detail.summary.status != .completed,
              let items = readyItems, items.fingerprint != nil else { return nil }
        return ImportSessionBlocks.applyBar(detail: detail, items: items)
    }

    func resume() {
        resumed = true
        let createdFor = createAccountFor
        createAccountFor = nil
        Task { await loadAccounts(createdFor: createdFor) }
        poll()
    }

    func pause() {
        resumed = false
        pollTask?.cancel()
    }

    func reload() async {
        pollTask?.cancel()
        await refresh()
    }

    private func poll() {
        guard resumed else { return }
        pollTask?.cancel()
        pollTask = Task { [weak self] in
            while let self, !Task.isCancelled {
                let busy: Bool?
                do {
                    busy = try await load(startedAt: generation)
                } catch {
                    if Task.isCancelled { return }
                    reportLoadFailure(error)
                    busy = false
                }
                guard busy == true else { return }
                try? await Task.sleep(for: Self.pollInterval)
            }
        }
    }

    private func load(startedAt: Int) async throws -> Bool? {
        let detail = try await store.getImportSession(sessionId: sessionId)
        var items: ImportItems?
        if detail.summary.classificationStatus == .classified {
            items = try await store.listImportItems(sessionId: sessionId)
        }
        var pending: [ImportPendingRow] = []
        if detail.counts.pending > 0 {
            pending = try await store.listImportPending(sessionId: sessionId)
        }
        guard generation == startedAt else { return nil }
        let attached = Set(detail.files.map(\.fileId))
        self.detail = detail
        self.items = items
        self.pending = pending
        seenDuplicates.formUnion((items?.rows ?? []).filter { !$0.nearMiss.isEmpty }.map(\.id))
        loadError = nil
        uploads.removeAll { upload in
            guard let fileId = upload.attachedFileId else { return false }
            return attached.contains(fileId)
        }
        return Self.isBusy(detail)
    }

    private static func isBusy(_ detail: ImportSessionDetail) -> Bool {
        let classification = detail.summary.classificationStatus
        return detail.files.contains { $0.status == .pending || $0.status == .processing }
            || classification == .waiting
            || classification == .running
            || (detail.summary.status == .reconciling && classification == .notStarted)
    }

    private func refresh() async {
        generation += 1
        do {
            if try await load(startedAt: generation) == true { poll() }
        } catch {
            reportLoadFailure(error)
        }
    }

    private func reportLoadFailure(_ error: Error) {
        if detail == nil {
            loadError = error.friendlyMessage
        } else {
            show(error.friendlyMessage)
        }
    }

    private func show(_ text: String) {
        toast = ImportToast(text: text)
    }

    private func loadAccounts(createdFor: String?) async {
        guard let accounts = try? await store.getAccountsList() else { return }
        self.accounts = accounts
        if let createdFor, let created = accounts.first(where: { !accountIdsBeforeCreate.contains($0.id) }) {
            accountChoices[createdFor] = created.id
        }
    }

    private func write(retry: (() -> Void)? = nil, _ block: () async throws -> Void) async {
        pollTask?.cancel()
        generation += 1
        writes += 1
        defer { writes -= 1 }
        do {
            try await block()
        } catch ApiError.AiFeaturesDisabled {
            if let retry {
                aiConsent.requestConsent(then: retry)
            } else {
                show(ApiError.AiFeaturesDisabled.friendlyMessage)
            }
        } catch {
            show(error.friendlyMessage)
        }
        await refresh()
    }

    func chooseAccount(rawAccount: String, accountId: String) {
        accountChoices[rawAccount] = accountId
    }

    func setSameAccount(rawAccount: String, same: Bool) {
        if same {
            separateAccounts.remove(rawAccount)
        } else {
            separateAccounts.insert(rawAccount)
        }
    }

    func prepareAccountCreation(rawAccount: String) async -> Bool {
        let known: [AccountItem]
        if let accounts {
            known = accounts
        } else {
            do {
                known = try await store.getAccountsList()
                accounts = known
            } catch {
                show(error.friendlyMessage)
                return false
            }
        }
        accountIdsBeforeCreate = Set(known.map(\.id))
        createAccountFor = rawAccount
        return true
    }

    func answerAccount(_ question: AccountQuestion, accountId: String?) async {
        let targets = question.targets(separate: separateAccounts.contains(question.rawAccount))
        let answer: ImportAnswer = accountId.map { .accountMap(accountId: $0) } ?? .accountExclude
        await write {
            for target in targets {
                try await store.answerImportAsk(sessionId: sessionId, askKey: target.askKey, answer: answer)
            }
        }
    }

    func chooseAsset(askKey: String, asset: ImportAssetCandidate) {
        assetChoices[askKey] = asset
    }

    func answerAsset(askKey: String, assetId: Int32?) async {
        let answer: ImportAnswer = assetId.map { .assetMap(assetId: $0) } ?? .assetExclude
        await write { try await store.answerImportAsk(sessionId: sessionId, askKey: askKey, answer: answer) }
    }

    func chooseDuplicate(rowId: String, choice: String) {
        duplicateChoices[rowId] = choice
    }

    func confirmDuplicate(rowId: String) async {
        guard let choice = duplicateChoices[rowId] else { return }
        if choice == Self.newTransactionChoice {
            await setVerb(itemIds: [rowId], verb: .add)
        } else {
            await setVerb(itemIds: [rowId], verb: .skip, candidateId: choice)
        }
    }

    func enterBalance(askKey: String, quantity: String) async {
        await write { try await store.answerImportAsk(sessionId: sessionId, askKey: askKey, answer: .enterBalance(quantity: quantity)) }
    }

    func answerUnlisted(askKey: String, delete: Bool) async {
        await write {
            try await store.answerImportAsk(sessionId: sessionId, askKey: askKey, answer: delete ? .deleteUnlisted : .keepUnlisted)
        }
    }

    func setVerb(itemIds: [String], verb: ImportItemVerb, candidateId: String? = nil) async {
        await write {
            try await store.setImportItemsVerb(sessionId: sessionId, itemIds: itemIds, verb: verb, candidateId: candidateId)
        }
    }

    func retryFile(fileId: String) async {
        await write(retry: { [weak self] in Task { await self?.retryFile(fileId: fileId) } }) {
            try await store.retryImportFile(sessionId: sessionId, fileId: fileId)
        }
    }

    func setDateOrder(fileId: String, order: ImportDateOrder) async {
        await write(retry: { [weak self] in Task { await self?.setDateOrder(fileId: fileId, order: order) } }) {
            try await store.setImportFileDateOrder(sessionId: sessionId, fileId: fileId, dateOrder: order)
        }
    }

    func removeFile(fileId: String) async {
        await write { try await store.detachImportFile(sessionId: sessionId, fileId: fileId) }
    }

    func retryClassification() async {
        await write { try await store.retryImportClassification(sessionId: sessionId) }
    }

    func upload(_ results: [PickResult]) {
        if let message = PickResult.failureMessage(results) {
            show(message)
        }
        for case let .picked(file) in results {
            let id = UUID().uuidString
            pickedFiles[id] = file
            uploads.append(LocalUpload(id: id, name: file.name))
            Task { await send(localId: id) }
        }
    }

    func retryUpload(localId: String) async {
        await send(localId: localId)
    }

    func dismissUpload(localId: String) {
        pickedFiles[localId] = nil
        uploads.removeAll { $0.id == localId }
    }

    private func updateUpload(_ localId: String, _ change: (inout LocalUpload) -> Void) {
        guard let index = uploads.firstIndex(where: { $0.id == localId }) else { return }
        change(&uploads[index])
    }

    private func send(localId: String) async {
        guard let file = pickedFiles[localId] else { return }
        updateUpload(localId) {
            $0.failed = false
            $0.error = nil
        }
        do {
            let fileId: String
            if let uploaded = uploads.first(where: { $0.id == localId })?.fileId {
                fileId = uploaded
            } else {
                fileId = try await store.uploadImportFile(data: file.data, mimeType: file.mimeType, fileName: file.name)
                updateUpload(localId) { $0.fileId = fileId }
            }
            try await store.attachImportFiles(sessionId: sessionId, fileIds: [fileId])
            pickedFiles[localId] = nil
            updateUpload(localId) { $0.attachedFileId = fileId }
        } catch ApiError.AiFeaturesDisabled {
            updateUpload(localId) {
                $0.failed = true
                $0.error = "AI features are off"
            }
            aiConsent.requestConsent { [weak self] in Task { await self?.send(localId: localId) } }
            return
        } catch {
            updateUpload(localId) {
                $0.failed = true
                $0.error = error.friendlyMessage
            }
            return
        }
        pollTask?.cancel()
        await refresh()
    }

    func apply() async {
        guard let items = readyItems, let fingerprint = items.fingerprint else { return }
        applying = true
        await write {
            defer { applying = false }
            try await store.applyImportSession(sessionId: sessionId, fingerprint: fingerprint)
            applied = ImportSessionBlocks.appliedCounts(items)
            await store.refreshTransactions()
        }
    }

    func delete() async -> Bool {
        pollTask?.cancel()
        generation += 1
        writes += 1
        defer { writes -= 1 }
        do {
            try await store.deleteImportSession(sessionId: sessionId)
            return true
        } catch {
            show(error.friendlyMessage)
            await refresh()
            return false
        }
    }
}
