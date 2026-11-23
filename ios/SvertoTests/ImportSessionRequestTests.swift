import Foundation
import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct ImportSessionRequestTests {
    @Test func confirmingADuplicateSkipsWithTheChosenCandidateOrAddsWhenNew() async {
        let store = RecordingImportStore(noHandle: .init())
        let model = ImportSessionViewModel(store: store, sessionId: "s")

        model.chooseDuplicate(rowId: "r1", choice: "ledger-1")
        await model.confirmDuplicate(rowId: "r1")
        model.chooseDuplicate(rowId: "r2", choice: ImportSessionViewModel.newTransactionChoice)
        await model.confirmDuplicate(rowId: "r2")

        #expect(store.verbCalls == [
            VerbCall(itemIds: ["r1"], verb: .skip, candidateId: "ledger-1"),
            VerbCall(itemIds: ["r2"], verb: .add, candidateId: nil),
        ])
    }

    @Test func linkingAnAccountAnswersEveryFileUnlessTheUserSplitsThem() async {
        let store = RecordingImportStore(noHandle: .init())
        let model = ImportSessionViewModel(store: store, sessionId: "s")
        let question = AccountQuestion(rawAccount: "Monzo", asks: [ask("k1", source: "f1"), ask("k2", source: "f2")])

        await model.answerAccount(question, accountId: "acc")
        #expect(store.answers == [
            AnswerCall(askKey: "k1", answer: .accountMap(accountId: "acc")),
            AnswerCall(askKey: "k2", answer: .accountMap(accountId: "acc")),
        ])

        store.reset()
        model.setSameAccount(rawAccount: "Monzo", same: false)
        await model.answerAccount(question, accountId: nil)
        #expect(store.answers == [AnswerCall(askKey: "k1", answer: .accountExclude)])
    }

    private func ask(_ key: String, source: String) -> AccountAsk {
        AccountAsk(askKey: key, sourceId: source, rawAccount: "Monzo", candidates: [], canExclude: true, answerCoversAllSources: false)
    }
}

nonisolated private struct VerbCall: Equatable {
    let itemIds: [String]
    let verb: ImportItemVerb
    let candidateId: String?
}

nonisolated private struct AnswerCall: Equatable {
    let askKey: String
    let answer: ImportAnswer
}

nonisolated private final class RecordingImportStore: AppStore, @unchecked Sendable {
    private let lock = NSLock()
    private var recordedVerbs: [VerbCall] = []
    private var recordedAnswers: [AnswerCall] = []

    var verbCalls: [VerbCall] { lock.withLock { recordedVerbs } }
    var answers: [AnswerCall] { lock.withLock { recordedAnswers } }

    func reset() {
        lock.withLock {
            recordedVerbs = []
            recordedAnswers = []
        }
    }

    override func setImportItemsVerb(sessionId: String, itemIds: [String], verb: ImportItemVerb, candidateId: String?) async throws {
        lock.withLock { recordedVerbs.append(VerbCall(itemIds: itemIds, verb: verb, candidateId: candidateId)) }
    }

    override func answerImportAsk(sessionId: String, askKey: String, answer: ImportAnswer) async throws {
        lock.withLock { recordedAnswers.append(AnswerCall(askKey: askKey, answer: answer)) }
    }

    override func getImportSession(sessionId: String) async throws -> ImportSessionDetail {
        throw ApiError.Network(reason: "offline")
    }
}
