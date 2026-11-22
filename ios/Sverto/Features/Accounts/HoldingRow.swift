import SvertoCore
import SwiftUI

struct HoldingRow: View {
    let holding: AccountHoldingItem
    let baseTicker: String
    let action: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var gainFraction: Double {
        holding.costBasis != 0 ? holding.unrealizedGains / holding.costBasis : 0
    }

    private var isProperty: Bool { HoldingAssetType.isProperty(assetTypeId: holding.assetTypeId) }

    private var title: String { isProperty ? holding.name : holding.ticker }

    private var monogramText: String {
        title.isEmpty ? "?" : String(title.prefix(1))
    }

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: Layout.rowSpacing) {
                HStack(spacing: Layout.rowSpacing) {
                    Monogram(text: monogramText, size: 36)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(title)
                            .font(.body.weight(.medium))
                            .foregroundStyle(.primary)
                            .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                        if !isProperty {
                            Text("\(AssetFormat.units(holding.units)) units")
                                .font(.subheadline)
                                .monospacedDigit()
                                .foregroundStyle(.secondary)
                        }
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: 8)
                    if !dynamicTypeSize.isAccessibilitySize {
                        amounts
                    }
                    Image(systemName: "chevron.forward")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.tertiary)
                        .accessibilityHidden(true)
                }
                if dynamicTypeSize.isAccessibilitySize {
                    amounts
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Layout.cardPadding)
            .padding(.vertical, Layout.rowSpacing)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
    }

    private var amounts: some View {
        VStack(alignment: dynamicTypeSize.isAccessibilitySize ? .leading : .trailing, spacing: 2) {
            Text(Money.format(holding.value, ticker: baseTicker))
                .font(.body.weight(.medium))
                .foregroundStyle(.primary)
            Text(PercentText.format(gainFraction))
                .font(.subheadline)
                .foregroundStyle(gainFraction >= 0 ? Color.gain : Color.loss)
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
    }
}
