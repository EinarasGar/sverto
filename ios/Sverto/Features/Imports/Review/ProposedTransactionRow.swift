import SvertoCore
import SwiftUI

extension ImportStagedRow {
    func supportingLine(withDate: Bool = false) -> String {
        [
            withDate ? ImportDates.date(date) : nil,
            categoryName ?? (typeLabel != description ? typeLabel : nil),
            accountName,
        ]
        .compactMap { $0 }
        .filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        .joined(separator: " · ")
    }
}

struct ProposedRowGlyph: View {
    let row: ImportStagedRow
    var size: CGFloat = Layout.rowGlyphSize
    var tint: Color = .brand

    var body: some View {
        IconTile(tint: tint, size: size) { side in
            if let icon = row.categoryIcon, !icon.isEmpty {
                LucideIcon(name: icon, size: side / 2)
            } else {
                Image(systemName: transactionTypeSymbol(row.typeKey))
            }
        }
    }
}

struct ProposedTransactionRow: View {
    let row: ImportStagedRow
    var showsVerb = true
    var showsDate = false
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var skipped: Bool { row.verb == .skip }
    private var showsChip: Bool { (showsVerb && row.verb != .add) || row.verbLocked }

    var body: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(spacing: Layout.rowSpacing))
        layout {
            ProposedRowGlyph(row: row, tint: skipped ? .secondary : .brand)
            VStack(alignment: .leading, spacing: 3) {
                Text(row.description)
                    .foregroundStyle(skipped ? .secondary : .primary)
                    .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                let supporting = row.supportingLine(withDate: showsDate)
                if !supporting.isEmpty {
                    Text(supporting)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                }
                if showsChip {
                    VerbChip(verb: row.verb, locked: row.verbLocked)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            if !dynamicTypeSize.isAccessibilitySize {
                Spacer(minLength: 8)
            }
            VStack(alignment: dynamicTypeSize.isAccessibilitySize ? .leading : .trailing, spacing: 2) {
                Text(row.amountDisplay)
                    .monospacedDigit()
                    .foregroundStyle(skipped ? .secondary : .primary)
                if let secondary = row.secondaryAmountDisplay {
                    Text(signedSecondaryAmount(secondary))
                        .font(.caption)
                        .monospacedDigit()
                        .foregroundStyle(.secondary)
                }
            }
            .lineLimit(1)
            .layoutPriority(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }
}

struct VerbChip: View {
    let verb: ImportRowVerb
    let locked: Bool

    var body: some View {
        HStack(spacing: 4) {
            if locked {
                Image(systemName: "lock.fill")
                    .font(.caption2.weight(.bold))
                    .imageScale(.small)
            }
            Text(verb.label)
                .font(.caption2.weight(.semibold))
        }
        .foregroundStyle(verb.tone.tint)
        .padding(.horizontal, 8)
        .padding(.vertical, 2)
        .background(verb.tone.fill, in: .rect(cornerRadius: 6))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(locked ? "\(verb.label), your choice" : verb.label)
    }
}
