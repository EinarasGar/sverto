import SwiftUI
import SvertoCore

struct HoldingsList: View {
    let holdings: [HoldingItem]
    let baseTicker: String
    let onSelect: (HoldingItem) -> Void

    var body: some View {
        Section("Holdings") {
            ForEach(holdings, id: \.assetId) { holding in
                Button {
                    onSelect(holding)
                } label: {
                    PortfolioHoldingRow(holding: holding, baseTicker: baseTicker)
                }
                .foregroundStyle(.primary)
            }
        }
    }
}

private struct PortfolioHoldingRow: View {
    let holding: HoldingItem
    let baseTicker: String
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var tint: Color { HoldingAssetType.tint(assetTypeId: holding.assetTypeId) }

    private var isProperty: Bool { HoldingAssetType.isProperty(assetTypeId: holding.assetTypeId) }

    private var unitsText: String {
        "\(AssetFormat.units(holding.units)) units"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Layout.rowSpacing) {
            HStack(spacing: Layout.rowSpacing) {
                Image(systemName: HoldingAssetType.systemImage(assetTypeId: holding.assetTypeId))
                    .font(.headline)
                    .foregroundStyle(tint)
                    .frame(width: 40, height: 40)
                    .background(tint.opacity(0.12), in: .rect(cornerRadius: 12))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text(holding.assetName)
                        .font(.body)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                    if !isProperty {
                        Text(holding.ticker)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                if !dynamicTypeSize.isAccessibilitySize {
                    Spacer(minLength: 8)
                    amounts
                }
            }
            if dynamicTypeSize.isAccessibilitySize {
                amounts
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 2)
        .contentShape(.rect)
        .accessibilityElement(children: .combine)
    }

    private var amounts: some View {
        VStack(alignment: dynamicTypeSize.isAccessibilitySize ? .leading : .trailing, spacing: 2) {
            Text(Money.format(holding.value, ticker: baseTicker))
                .font(.body.weight(.medium))
            if !isProperty {
                Text(unitsText)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
    }
}
