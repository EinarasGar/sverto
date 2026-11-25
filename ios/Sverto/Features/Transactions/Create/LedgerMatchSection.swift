import SvertoCore
import SwiftUI

struct LedgerMatchSection: View {
    @Bindable var controller: LedgerMatchController

    private var choosesBetweenMatches: Bool { controller.matches.count > 1 }

    var body: some View {
        if !controller.matches.isEmpty {
            Section {
                ForEach(controller.matches, id: \.id) { candidate in
                    if choosesBetweenMatches {
                        let isSelected = candidate.id == controller.mergeTargetId
                        Button { controller.select(candidate.id) } label: {
                            LedgerMatchRow(candidate: candidate, isSelected: isSelected)
                        }
                        .foregroundStyle(.primary)
                        .accessibilityAddTraits(isSelected ? .isSelected : [])
                    } else {
                        LedgerMatchRow(candidate: candidate)
                    }
                }
                Picker("Receipt", selection: $controller.mergesIntoMatch) {
                    Text("Merge into it").tag(true)
                    Text("It's a different purchase").tag(false)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .sensoryFeedback(.selection, trigger: controller.mergesIntoMatch)
            } header: {
                Text("Already in your ledger")
            } footer: {
                Text(controller.mergesIntoMatch
                    ? "The receipt's lines and date replace it; its account stays."
                    : "The receipt is saved as a new transaction.")
            }
        }
    }
}

private struct LedgerMatchRow: View {
    let candidate: TransactionCandidate
    var isSelected: Bool?

    private var detail: String {
        [candidate.accountName, ImportDates.date(candidate.date)].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var body: some View {
        HStack(spacing: Layout.rowSpacing) {
            IconTile { side in
                if let icon = candidate.categoryIcon, !icon.isEmpty {
                    LucideIcon(name: icon, size: side / 2)
                } else {
                    Image(systemName: TransactionTypeCatalog.regular.systemImage)
                }
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(candidate.description ?? "No description")
                Text(detail)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
            .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 8)
            Text(candidate.amountDisplay)
                .monospacedDigit()
            if let isSelected {
                Image(systemName: "checkmark")
                    .font(.body.weight(.semibold))
                    .foregroundStyle(.tint)
                    .opacity(isSelected ? 1 : 0)
                    .accessibilityHidden(true)
            }
        }
        .accessibilityElement(children: .combine)
    }
}
