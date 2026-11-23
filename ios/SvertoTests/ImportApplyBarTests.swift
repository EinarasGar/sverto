import SvertoCore
import Testing
@testable import Sverto

@MainActor
struct ImportApplyBarTests {
    @Test func openDuplicateTurnsApplyIntoANavigatorToIt() throws {
        let candidate = ImportCandidate(
            id: "ledger-1",
            inLedger: true,
            amend: false,
            leavesMovementUnrecorded: false,
            date: 0,
            description: "Coffee",
            accountName: "Monzo",
            amountDisplay: "-£3.00",
            parts: []
        )

        let state = ImportSessionBlocks.applyBar(detail: detail, items: items([row("r1"), row("r2", nearMiss: [candidate])]))

        guard case let .navigate(_, target) = state else { throw ApplyBarMismatch() }
        #expect(target == .block(SessionBlock.duplicateId("r2")))
    }

    @Test func onlyBalancedAddsOnlyImportsSkipTheConfirmation() throws {
        let balanced = try confirm(of: ImportSessionBlocks.applyBar(detail: detail, items: items([row("r1"), row("r2")], checks: [check(off: 0)])))
        #expect(balanced == nil)

        let offBalance = try confirm(of: ImportSessionBlocks.applyBar(detail: detail, items: items([row("r1")], checks: [check(off: 12)])))
        #expect(offBalance?.risky == true)

        let deleting = try confirm(of: ImportSessionBlocks.applyBar(detail: detail, items: items([row("r1"), row("r2", verb: .delete)])))
        #expect(deleting?.risky == true)
    }

    private func confirm(of state: ApplyBarState) throws -> ApplyConfirm? {
        guard case let .apply(_, _, confirm) = state else { throw ApplyBarMismatch() }
        return confirm
    }

    private struct ApplyBarMismatch: Error {}

    private var detail: ImportSessionDetail {
        ImportSessionDetail(
            summary: ImportSessionSummary(id: "s", title: "Monzo", status: .proposalReady, classificationStatus: .classified, createdAt: 0),
            classificationError: nil,
            sources: [],
            asks: [],
            counts: ImportCounts(pending: 0)
        )
    }

    private func items(_ rows: [ImportStagedRow], checks: [ImportBalanceCheck] = []) -> ImportItems {
        ImportItems(
            ready: true,
            fingerprint: "fp",
            rows: rows,
            balanceChecks: ImportBalanceChecks(checks: checks, unchecked: 0, entered: []),
            unstageable: []
        )
    }

    private func row(_ id: String, verb: ImportRowVerb = .add, nearMiss: [ImportCandidate] = []) -> ImportStagedRow {
        ImportStagedRow(
            id: id,
            kind: .movement,
            verb: verb,
            verbLocked: false,
            askKey: nil,
            amendRefusal: nil,
            typeKey: "regular",
            typeLabel: "Regular",
            date: 1_700_000_000,
            description: "Coffee",
            accountId: "a",
            assetId: 1,
            accountName: "Monzo",
            amountDisplay: "-£3.00",
            secondaryAmountDisplay: nil,
            categoryName: nil,
            categoryIcon: nil,
            nearMiss: nearMiss
        )
    }

    private func check(off: Double) -> ImportBalanceCheck {
        ImportBalanceCheck(
            accountId: "a",
            assetId: 1,
            accountName: "Monzo",
            ticker: "GBP",
            firstDate: 0,
            lastDate: 0,
            balancesChecked: 2,
            startingOffBy: 0,
            startingOffByDisplay: "£0.00",
            finalOffBy: off,
            finalOffByDisplay: "£\(off)",
            changes: [],
            unlisted: []
        )
    }
}
