import SwiftUI

struct CurrencyChip: View {
    let ticker: String
    var showsChevron = true

    var body: some View {
        HStack(spacing: 4) {
            Text(ticker)
            if showsChevron {
                Image(systemName: "chevron.down")
                    .font(.caption.weight(.semibold))
                    .accessibilityHidden(true)
            }
        }
        .font(.subheadline.weight(.semibold))
        .foregroundStyle(showsChevron ? .primary : .secondary)
        .padding(.horizontal, 12)
        .frame(minHeight: 34)
        .background(Capsule().fill(.fill.tertiary))
        .frame(minHeight: 44)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Currency \(ticker)")
    }
}

#Preview {
    VStack(spacing: 16) {
        CurrencyChip(ticker: "USD")
        CurrencyChip(ticker: "EUR", showsChevron: false)
    }
    .padding()
}
