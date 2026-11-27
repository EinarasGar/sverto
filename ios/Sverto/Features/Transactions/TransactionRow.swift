import SwiftUI
import SvertoCore

struct TransactionRow: View {
    let transaction: TransactionListItem
    var isSelected = false
    var isHighlighted = false
    let onTap: () -> Void
    var onLongPress: (() -> Void)? = nil

    @State private var isPressed = false
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.horizontalSizeClass) private var sizeClass

    private var subtitle: String {
        transaction.categoryName.isEmpty ? transaction.typeLabel : transaction.categoryName
    }

    private var isDimmed: Bool {
        transaction.visibility == .ghost && !isSelected
    }

    var body: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: Layout.rowSpacing))
            : AnyLayout(HStackLayout(spacing: Layout.rowSpacing))
        layout {
            leading
                .dynamicTypeSize(dynamicTypeSize.isAccessibilitySize ? .large : dynamicTypeSize)
            VStack(alignment: .leading, spacing: 2) {
                Text(transaction.description)
                    .font(.body)
                    .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                if !subtitle.isEmpty {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            if !dynamicTypeSize.isAccessibilitySize {
                Spacer(minLength: 8)
            }
            TransactionAmount(transaction: transaction, lineLimit: dynamicTypeSize.isAccessibilitySize ? nil : 1)
                .fixedSize(horizontal: false, vertical: true)
                .layoutPriority(sizeClass == .regular ? 1 : 0)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 4)
        .opacity(isDimmed ? 0.55 : 1)
        .contentShape(.rect)
        .hoverEffect(.highlight)
        .onTapGesture(perform: onTap)
        .onLongPressGesture(perform: { onLongPress?() }, onPressingChanged: { isPressed = $0 })
        .listRowBackground(rowBackground)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityAddTraits(isSelected || isHighlighted ? .isSelected : [])
        .accessibilityHint(onLongPress == nil ? "" : "Long press to select")
        .accessibilityActions {
            if let onLongPress {
                Button(isSelected ? "Deselect" : "Select", action: onLongPress)
            }
        }
    }

    @ViewBuilder
    private var leading: some View {
        if isSelected {
            Image(systemName: "checkmark")
                .font(.system(size: 16, weight: .bold))
                .foregroundStyle(Color.onAccent)
                .frame(width: 40, height: 40)
                .background(.tint, in: .circle)
                .accessibilityLabel("Selected")
        } else {
            TransactionGlyph(transaction: transaction)
        }
    }

    private var rowBackground: Color {
        if isSelected {
            return Color.accentColor.opacity(0.14)
        }
        return isPressed || isHighlighted ? Color(.systemFill) : .cardFill
    }
}
