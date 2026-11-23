import SvertoCore
import SwiftUI

struct OutcomeCard: View {
    let state: OutcomeState

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(state.headline)
                .font(.title2.weight(.bold))
                .accessibilityAddTraits(.isHeader)
            if let breakdown = state.breakdown {
                Text(breakdown)
            }
            Text(state.scope)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            if let balance = state.balance {
                Label {
                    Text(balance.text)
                } icon: {
                    Image(systemName: balance.ok ? "checkmark.circle.fill" : "exclamationmark.circle.fill")
                }
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(balance.ok ? Color.gain : Color.loss)
                .padding(.top, 4)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Layout.cardPadding + 4)
        .background(Color.brand.opacity(0.1), in: .rect(cornerRadius: Layout.cardRadius))
        .accessibilityElement(children: .combine)
    }
}

struct DecisionNavCard: View {
    let items: [DecisionNavItem]
    let onOpen: (DecisionNavItem) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionHeader("Needs your decision")
            GroupedCard {
                VStack(spacing: 0) {
                    ForEach(Array(items.enumerated()), id: \.element) { index, item in
                        if index > 0 {
                            RowDivider(leadingInset: Layout.cardPadding + 20)
                        }
                        Button { onOpen(item) } label: {
                            HStack(spacing: 12) {
                                Circle()
                                    .fill(Color(.systemOrange))
                                    .frame(width: 8, height: 8)
                                    .accessibilityHidden(true)
                                Text(item.label)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                if let progress = item.progress {
                                    CountLabel(progress)
                                }
                                Image(systemName: "chevron.forward")
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(.tertiary)
                                    .accessibilityHidden(true)
                            }
                            .padding(.horizontal, Layout.cardPadding)
                            .frame(minHeight: 50)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.cardRow)
                        .accessibilityHint("Scrolls to it")
                    }
                }
            }
        }
    }
}

struct QuestionsHeading: View {
    let count: Int

    var body: some View {
        Text(count == 1 ? "1 question to finish this import" : "\(count) questions to finish this import")
            .font(.title2.weight(.bold))
            .fixedSize(horizontal: false, vertical: true)
            .accessibilityAddTraits(.isHeader)
            .padding(.top, 8)
    }
}

struct PreviewRowsCard: View {
    let rows: [ImportStagedRow]
    let total: Int
    let onRowTap: (ImportStagedRow) -> Void
    let onViewAll: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            SectionHeader("Transactions", trailing: { AnyView(CountLabel(ImportCopy.formatCount(total))) })
            GroupedCard {
                VStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                        if index > 0 {
                            RowDivider(leadingInset: Layout.cardPadding + Layout.rowGlyphSize + Layout.rowSpacing)
                        }
                        Button { onRowTap(row) } label: {
                            ProposedTransactionRow(row: row, showsDate: true)
                                .padding(.horizontal, Layout.cardPadding)
                                .padding(.vertical, 10)
                        }
                        .buttonStyle(.cardRow)
                    }
                    if total > rows.count {
                        RowDivider()
                        Button(action: onViewAll) {
                            HStack {
                                Text("View all \(ImportCopy.formatCount(total)) transactions")
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                Image(systemName: "chevron.forward")
                                    .font(.footnote.weight(.semibold))
                                    .accessibilityHidden(true)
                            }
                            .foregroundStyle(.tint)
                            .padding(.horizontal, Layout.cardPadding)
                            .frame(minHeight: 50)
                            .contentShape(.rect)
                        }
                        .buttonStyle(.cardRow)
                    }
                }
            }
        }
    }
}
