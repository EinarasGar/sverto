import SvertoCore
import SwiftUI

struct AccountCard: View {
    let account: AccountListItem
    let baseTicker: String
    var isHighlighted = false
    var showsChevron = true
    let action: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        Button(action: action) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: 12) {
                        HStack(alignment: .top, spacing: 14) {
                            AccountTypeBadge(typeId: account.accountTypeId)
                            identity
                                .frame(maxWidth: .infinity, alignment: .leading)
                            if showsChevron {
                                chevron
                            }
                        }
                        trailing
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                } else {
                    HStack(spacing: 14) {
                        AccountTypeBadge(typeId: account.accountTypeId)
                        identity
                        Spacer(minLength: 8)
                        trailing
                        if showsChevron {
                            chevron
                        }
                    }
                }
            }
            .padding(.vertical, 4)
        }
        .listRowBackground(isHighlighted ? Color(.systemFill) : nil)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(isHighlighted ? .isSelected : [])
    }

    private var identity: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(account.name)
                .font(.body.weight(.medium))
                .foregroundStyle(Color.primary)
                .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
            Text(AccountTypeVisuals.label(account.accountTypeId))
                .font(.subheadline)
                .foregroundStyle(Color.secondary)
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    private var chevron: some View {
        Image(systemName: "chevron.forward")
            .font(.footnote.weight(.semibold))
            .foregroundStyle(Color(uiColor: .tertiaryLabel))
            .accessibilityHidden(true)
    }

    private var amount: Double? {
        if let property = account.property {
            return property.equityShare
        }
        return account.balance
    }

    @ViewBuilder
    private var trailing: some View {
        if let balance = amount {
            VStack(alignment: dynamicTypeSize.isAccessibilitySize ? .leading : .trailing, spacing: 2) {
                Text(Money.format(balance, ticker: baseTicker))
                    .font(.body.weight(.semibold))
                    .monospacedDigit()
                    .foregroundStyle(Color.primary)
                    .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                    .fixedSize(horizontal: !dynamicTypeSize.isAccessibilitySize, vertical: true)
                if account.property != nil {
                    Text("Equity")
                        .font(.caption)
                        .foregroundStyle(Color.secondary)
                } else if AccountTypeVisuals.isInvestment(account.accountTypeId), let gain = account.unrealizedGain {
                    Text(gainText(gain: gain, balance: balance))
                        .font(.caption)
                        .monospacedDigit()
                        .foregroundStyle(gain >= 0 ? Color.gain : Color.loss)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        } else {
            SkeletonBlock(width: 80, height: 20)
        }
    }

    private func gainText(gain: Double, balance: Double) -> String {
        let fraction = balance > 0 ? gain / balance : 0
        return "\(Money.format(gain, ticker: baseTicker, signed: true)) (\(PercentText.format(abs(fraction), signed: false)))"
    }
}
