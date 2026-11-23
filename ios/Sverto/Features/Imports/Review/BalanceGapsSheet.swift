import SvertoCore
import SwiftUI

struct BalanceGapsSheet: View {
    let block: BalanceCheckBlock
    let enabled: Bool
    let onSetAdjustments: (_ rowIds: [String], _ add: Bool) -> Void
    let onAnswerUnlisted: (_ askKey: String, _ delete: Bool) -> Void

    @Environment(\.dismiss) private var dismiss

    private var check: ImportBalanceCheck { block.check }
    private var offers: [ImportStagedRow] {
        check.changes.compactMap { Self.adjustmentOffer(for: $0, adjustments: block.adjustments) }
    }
    private var added: [ImportStagedRow] { block.adjustments.filter { $0.verb == .add } }
    private var notListed: [ImportUnlistedMovement] { check.unlisted.filter { !$0.mayBookLater } }
    private var mayBookLater: [ImportUnlistedMovement] { check.unlisted.filter(\.mayBookLater) }

    static func adjustmentOffer(for change: ImportBalanceChange, adjustments: [ImportStagedRow]) -> ImportStagedRow? {
        guard !change.explained, change.after != change.through else { return nil }
        return adjustments.first { $0.verb != .add && utcDay($0.date) == utcDay(change.through) }
    }

    private static func utcDay(_ epochSeconds: Int64) -> Int64 {
        let secondsPerDay: Int64 = 86_400
        return epochSeconds >= 0 ? epochSeconds / secondsPerDay : (epochSeconds - secondsPerDay + 1) / secondsPerDay
    }

    var body: some View {
        NavigationStack {
            List {
                if !check.changes.isEmpty {
                    Section {
                        ForEach(check.changes, id: \.self) { change in
                            GapRow(change: change, offer: Self.adjustmentOffer(for: change, adjustments: block.adjustments), enabled: enabled) {
                                onSetAdjustments([$0], true)
                            }
                        }
                    } header: {
                        Text("Gaps")
                    } footer: {
                        Text("Nothing in your ledger explains these changes between statement balances. Add the statements for those dates, or add an adjustment that closes the gap.")
                    }
                    if offers.count > 1 {
                        Section {
                            Button("Add all \(offers.count) adjustments") {
                                onSetAdjustments(offers.map(\.id), true)
                            }
                            .disabled(!enabled)
                        }
                    }
                }
                if !notListed.isEmpty {
                    Section("In your ledger but not on this statement") {
                        UnlistedGroup(rows: notListed, enabled: enabled, onAnswer: onAnswerUnlisted)
                    }
                }
                if !mayBookLater.isEmpty {
                    Section {
                        UnlistedGroup(rows: mayBookLater, enabled: enabled, onAnswer: nil)
                    } header: {
                        Text("May book later")
                    } footer: {
                        Text("Dated in the statement's last days, so the bank may book these on the next statement.")
                    }
                }
                if !added.isEmpty {
                    Section("Adjustments you added") {
                        ForEach(added, id: \.id) { row in
                            HStack {
                                Text("\(row.amountDisplay) on \(ImportDates.date(row.date))")
                                    .monospacedDigit()
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                Button("Remove") { onSetAdjustments([row.id], false) }
                                    .buttonStyle(.borderless)
                                    .disabled(!enabled)
                                    .accessibilityLabel("Remove adjustment of \(row.amountDisplay) on \(ImportDates.date(row.date))")
                            }
                        }
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle(check.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

private struct GapRow: View {
    let change: ImportBalanceChange
    let offer: ImportStagedRow?
    let enabled: Bool
    let onAdd: (String) -> Void

    private var period: String { ImportDates.range(change.after, change.through) }

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(change.after == change.through ? "\(ImportDates.date(change.through)): two statements disagree" : period)
                Text(change.changeDisplay)
                    .font(.subheadline)
                    .monospacedDigit()
                    .foregroundStyle(.secondary)
                if change.awaitsAnswer {
                    Text("A possible duplicate on these dates is still open; it may settle this.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            if let offer {
                Button("Add") { onAdd(offer.id) }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .disabled(!enabled)
                    .accessibilityLabel("Add adjustment of \(offer.amountDisplay), \(period)")
            }
        }
    }
}
