import SwiftUI
import SvertoCore

struct HoldingsList: View {
    let state: PortfolioState
    let onSelectHolding: (HoldingItem) -> Void
    let onSelectLiability: (LiabilityItem) -> Void

    var body: some View {
        if !state.holdings.isEmpty {
            Section {
                ForEach(state.holdings, id: \.assetId) { holding in
                    Button {
                        onSelectHolding(holding)
                    } label: {
                        PortfolioRow(holding: holding, baseTicker: state.baseTicker)
                    }
                    .foregroundStyle(.primary)
                }
            } header: {
                SectionTotalHeader(title: "Holdings", total: Money.format(state.holdingsTotal, ticker: state.baseTicker))
            }
        }
        if !state.liabilities.isEmpty {
            Section {
                ForEach(state.liabilities, id: \.accountId) { liability in
                    Button {
                        onSelectLiability(liability)
                    } label: {
                        PortfolioRow(liability: liability, baseTicker: state.baseTicker)
                    }
                    .foregroundStyle(.primary)
                }
            } header: {
                SectionTotalHeader(title: "Liabilities", total: Money.format(state.liabilitiesTotal, ticker: state.baseTicker))
            }
        }
    }
}

private struct SectionTotalHeader: View {
    let title: String
    let total: String

    var body: some View {
        HStack {
            Text(title)
            Spacer()
            Text(total)
                .monospacedDigit()
        }
    }
}

private struct PortfolioRow: View {
    let systemImage: String
    let tint: Color
    let title: String
    let subtitle: String?
    let value: String
    let detail: String?
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    init(holding: HoldingItem, baseTicker: String) {
        let isProperty = HoldingAssetType.isProperty(assetTypeId: holding.assetTypeId)
        tint = HoldingAssetType.tint(assetTypeId: holding.assetTypeId)
        title = holding.assetName
        value = Money.format(holding.value, ticker: baseTicker)
        if let property = holding.property {
            let kind: String
            switch property.kind {
            case .realEstate:
                systemImage = "house"
                kind = "Property"
            case .vehicle:
                systemImage = "car"
                kind = "Vehicle"
            }
            if property.loanBalance == 0 {
                subtitle = kind
                detail = nil
            } else {
                subtitle = "\(kind) · Value \(Money.format(property.assetValue, ticker: baseTicker))"
                detail = "Loan \(Money.format(abs(property.loanBalance), ticker: baseTicker))"
            }
        } else {
            systemImage = HoldingAssetType.systemImage(assetTypeId: holding.assetTypeId)
            subtitle = isProperty ? nil : holding.ticker
            detail = isProperty ? nil : "\(AssetFormat.units(holding.units)) units"
        }
    }

    init(liability: LiabilityItem, baseTicker: String) {
        systemImage = AccountTypeVisuals.systemImage(liability.accountTypeId)
        tint = AccountTypeVisuals.tint(liability.accountTypeId)
        title = liability.accountName
        subtitle = AccountTypeVisuals.label(liability.accountTypeId)
        value = Money.format(liability.value, ticker: baseTicker)
        detail = nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Layout.rowSpacing) {
            HStack(spacing: Layout.rowSpacing) {
                Image(systemName: systemImage)
                    .font(.headline)
                    .foregroundStyle(tint)
                    .frame(width: 40, height: 40)
                    .background(tint.opacity(0.12), in: .rect(cornerRadius: 12))
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title)
                        .font(.body)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                    if let subtitle {
                        Text(subtitle)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
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
            Text(value)
                .font(.body.weight(.medium))
            if let detail {
                Text(detail)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        }
        .monospacedDigit()
        .fixedSize(horizontal: false, vertical: true)
    }
}
