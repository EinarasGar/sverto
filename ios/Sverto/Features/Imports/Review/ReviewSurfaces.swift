import SwiftUI

struct ReviewCard<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        GroupedCard {
            VStack(alignment: .leading, spacing: 12) {
                content
            }
            .padding(Layout.cardPadding)
        }
    }
}

struct DetailLine: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.subheadline)
            .foregroundStyle(.secondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}

struct CountLabel: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.subheadline)
            .monospacedDigit()
            .foregroundStyle(.secondary)
    }
}

struct ExpandableCard<Content: View>: View {
    let title: String
    @Binding var isExpanded: Bool
    private let content: Content
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(title: String, isExpanded: Binding<Bool>, @ViewBuilder content: () -> Content) {
        self.title = title
        _isExpanded = isExpanded
        self.content = content()
    }

    var body: some View {
        GroupedCard {
            VStack(spacing: 0) {
                Button {
                    withAnimation(reduceMotion ? nil : .snappy) { isExpanded.toggle() }
                } label: {
                    HStack(spacing: 12) {
                        Text(title)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .fixedSize(horizontal: false, vertical: true)
                        Image(systemName: "chevron.down")
                            .font(.footnote.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .rotationEffect(.degrees(isExpanded ? 180 : 0))
                            .accessibilityHidden(true)
                    }
                    .padding(Layout.cardPadding)
                    .frame(minHeight: 52)
                    .contentShape(.rect)
                }
                .buttonStyle(.cardRow)
                .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")
                if isExpanded {
                    content
                }
            }
        }
    }
}

struct SimpleRow: View {
    let title: String
    let detail: String
    var trailing: String? = nil

    var body: some View {
        VStack(spacing: 0) {
            RowDivider()
            HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .lineLimit(1)
                    Text(detail)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if let trailing {
                    Text(trailing)
                        .monospacedDigit()
                }
            }
            .padding(.horizontal, Layout.cardPadding)
            .padding(.vertical, 12)
        }
        .accessibilityElement(children: .combine)
    }
}

struct CardRowButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(configuration.isPressed ? Color(.systemFill) : .clear)
    }
}

extension ButtonStyle where Self == CardRowButtonStyle {
    static var cardRow: CardRowButtonStyle { CardRowButtonStyle() }
}

struct LabelWithChip<Label: View, Chip: View>: View {
    private let label: Label
    private let chip: Chip
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    init(@ViewBuilder label: () -> Label, @ViewBuilder chip: () -> Chip) {
        self.label = label()
        self.chip = chip()
    }

    var body: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .leading, spacing: 8))
            : AnyLayout(HStackLayout(alignment: .center, spacing: 12))
        layout {
            label
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity, alignment: .leading)
            chip
                .fixedSize()
        }
    }
}
