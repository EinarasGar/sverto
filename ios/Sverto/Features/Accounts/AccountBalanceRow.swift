import SvertoCore
import SwiftUI

struct AccountBalanceRow: View {
    let balance: AccountCurrencyBalance
    let baseTicker: String
    let action: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var tint: Color { HoldingAssetType.tint(assetTypeId: HoldingAssetType.currency.rawValue) }

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: Layout.rowSpacing) {
                HStack(spacing: Layout.rowSpacing) {
                    Image(systemName: HoldingAssetType.systemImage(assetTypeId: HoldingAssetType.currency.rawValue))
                        .font(.headline)
                        .foregroundStyle(tint)
                        .frame(width: 40, height: 40)
                        .background(tint.opacity(0.12), in: .rect(cornerRadius: 12))
                        .accessibilityHidden(true)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(balance.ticker)
                            .font(.body.weight(.medium))
                            .foregroundStyle(.primary)
                        Text(balance.name)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
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
            Text(Money.format(balance.units, ticker: balance.ticker))
                .font(.body.weight(.medium))
                .foregroundStyle(.primary)
            if balance.ticker != baseTicker, let value = balance.value {
                Text(Money.format(value, ticker: baseTicker))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
    }
}
