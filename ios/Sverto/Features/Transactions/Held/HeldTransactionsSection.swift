import SvertoCore
import SwiftUI

struct HeldTransactionsSection: View {
    let model: HeldTransactionsViewModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header
            ForEach(model.items, id: \.id) { held in
                HeldTransactionCard(
                    held: held,
                    deciding: model.decidingId == held.id,
                    enabled: model.decidingId == nil
                ) { decision in
                    Task { await model.decide(held, decision: decision) }
                }
                .transition(.opacity)
            }
        }
        .animation(reduceMotion ? nil : .snappy, value: model.items.map(\.id))
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "square.on.square")
                .font(.body.weight(.semibold))
                .foregroundStyle(.brand)
                .frame(width: 34, height: 34)
                .background(Color.brand.opacity(0.14), in: .circle)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text("Possible duplicates (\(model.items.count))")
                    .font(.subheadline.weight(.semibold))
                Text("Bank transactions we think you already have")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}
