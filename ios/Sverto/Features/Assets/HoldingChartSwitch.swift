import SwiftUI

struct HoldingChartSwitch: View {
    @Binding var selection: HoldingChartSeries

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 2) {
            ForEach(HoldingChartSeries.allCases, id: \.self) { series in
                option(series)
            }
        }
        .sensoryFeedback(.selection, trigger: selection)
    }

    private func option(_ series: HoldingChartSeries) -> some View {
        let isSelected = series == selection
        return Button {
            withAnimation(reduceMotion ? nil : .spring(response: 0.35, dampingFraction: 0.78)) {
                selection = series
            }
        } label: {
            HStack(spacing: 0) {
                Image(systemName: series.systemImage)
                LabelRevealLayout(progress: isSelected ? 1 : 0) {
                    Text(series.label)
                        .lineLimit(1)
                        .padding(.leading, 6)
                }
                .modifier(LabelFade(progress: isSelected ? 1 : 0))
            }
            .font(.subheadline.weight(.semibold))
            .foregroundStyle(isSelected ? Color.onAccent : .secondary)
            .padding(.horizontal, 12)
            .frame(minWidth: 44, minHeight: 34)
            .background(shape(series, isSelected: isSelected).fill(isSelected ? AnyShapeStyle(.tint) : AnyShapeStyle(.fill.tertiary)))
            .frame(minHeight: 44)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(series.label)
        .accessibilityAddTraits(isSelected ? .isSelected : [])
    }

    private func shape(_ series: HoldingChartSeries, isSelected: Bool) -> UnevenRoundedRectangle {
        let outer: CGFloat = 17
        let inner: CGFloat = isSelected ? outer : 6
        let leading = series == HoldingChartSeries.allCases.first ? outer : inner
        let trailing = series == HoldingChartSeries.allCases.last ? outer : inner
        return UnevenRoundedRectangle(
            topLeadingRadius: leading,
            bottomLeadingRadius: leading,
            bottomTrailingRadius: trailing,
            topTrailingRadius: trailing,
            style: .continuous
        )
    }
}

// Takes up a growing share of the label's width while always drawing the label whole, so the button can open around it.
private struct LabelRevealLayout: SwiftUI.Layout {
    var progress: CGFloat

    var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }

    func sizeThatFits(proposal _: ProposedViewSize, subviews: Subviews, cache _: inout ()) -> CGSize {
        let label = subviews[0].sizeThatFits(.unspecified)
        return CGSize(width: label.width * max(0, progress), height: label.height)
    }

    func placeSubviews(in bounds: CGRect, proposal _: ProposedViewSize, subviews: Subviews, cache _: inout ()) {
        subviews[0].place(at: bounds.origin, anchor: .topLeading, proposal: .unspecified)
    }
}

// The label stays hidden until the button has opened this far, so it never shows outside the button.
private struct LabelFade: ViewModifier, Animatable {
    var progress: CGFloat

    var animatableData: CGFloat {
        get { progress }
        set { progress = newValue }
    }

    func body(content: Content) -> some View {
        content.opacity(max(0, (progress - 0.8) / 0.2))
    }
}

#Preview {
    @Previewable @State var selection = HoldingChartSeries.value
    HoldingChartSwitch(selection: $selection)
        .padding()
}
