import SvertoCore
import SwiftUI

struct ProposedRowSheet: View {
    let row: ImportStagedRow
    let onVerb: (ImportItemVerb) -> Void
    let onKeep: (_ askKey: String) -> Void

    @Environment(\.dismiss) private var dismiss

    private var notes: [String] {
        [row.kind.note, row.nearMiss.isEmpty ? nil : "Possible duplicate: settle it on the import screen."].compactMap { $0 }
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    header
                } footer: {
                    if !notes.isEmpty || row.amendRefusal != nil {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(notes, id: \.self) { Text($0) }
                            if let refusal = row.amendRefusal {
                                Text(refusal.note)
                                    .foregroundStyle(.loss)
                            }
                        }
                    }
                }
                actions
                Section {
                    LabeledContent("Date", value: ImportDates.date(row.date))
                    LabeledContent("Account", value: row.accountName)
                    LabeledContent(row.categoryName == nil ? "Type" : "Category", value: row.categoryName ?? row.typeLabel)
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("Proposed transaction")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }

    private var header: some View {
        HStack(spacing: 16) {
            ProposedRowGlyph(row: row, size: 52)
            VStack(alignment: .leading, spacing: 2) {
                Text(row.description)
                    .font(.headline)
                Text(row.amountDisplay)
                    .font(.title2.weight(.semibold))
                    .monospacedDigit()
                if let secondary = row.secondaryAmountDisplay {
                    Text(signedSecondaryAmount(secondary))
                        .font(.subheadline)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
    }

    @ViewBuilder
    private var actions: some View {
        if row.verb == .delete {
            if let askKey = row.askKey {
                Section {
                    Button("Keep it") { onKeep(askKey) }
                } footer: {
                    Text("This ledger transaction isn't on the statement, so the import deletes it.")
                }
            }
        } else {
            Section {
                Picker("In this import", selection: Binding<ImportRowVerb?>(
                    get: { row.verb == .add || row.verb == .skip ? row.verb : nil },
                    set: { verb in
                        guard let verb, verb != row.verb else { return }
                        onVerb(verb == .skip ? .skip : .add)
                    }
                )) {
                    Text("Add").tag(ImportRowVerb?.some(.add))
                    Text("Skip").tag(ImportRowVerb?.some(.skip))
                }
                .pickerStyle(.segmented)
                .listRowBackground(Color.clear)
                .listRowInsets(EdgeInsets())
                if row.kind == .joinedTransfer {
                    Button("Keep apart") { onVerb(.apart) }
                }
                if row.verbLocked {
                    Button("Undo my choice") { onVerb(.auto) }
                }
            } header: {
                Text(row.verbLocked ? "In this import · your choice" : "In this import")
            }
        }
    }
}
