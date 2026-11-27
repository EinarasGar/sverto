import SwiftUI

struct MetricItem: Identifiable {
    let id = UUID()
    var label: String
    var value: String
    var valueColor: Color?

    init(label: String, value: String, valueColor: Color? = nil) {
        self.label = label
        self.value = value
        self.valueColor = valueColor
    }
}

struct MetricGrid: View {
    private let items: [MetricItem]
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var width: CGFloat = 0

    init(items: [MetricItem]) {
        self.items = items
    }

    private var columns: Int {
        if dynamicTypeSize.isAccessibilitySize { 1 } else if sizeClass == .regular, width >= 560 { 4 } else { 2 }
    }

    private var rows: [[MetricItem]] {
        stride(from: 0, to: items.count, by: columns).map { Array(items[$0..<min($0 + columns, items.count)]) }
    }

    var body: some View {
        VStack(spacing: 8) {
            ForEach(rows, id: \.first?.id) { row in
                HStack(alignment: .top, spacing: 8) {
                    ForEach(row) { item in
                        MetricTile(item: item)
                    }
                    if row.count < columns {
                        Color.clear.frame(maxWidth: .infinity)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
            }
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width.rounded() } action: { width = $0 }
    }
}

private struct MetricTile: View {
    let item: MetricItem

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(item.label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(item.value)
                .font(.headline)
                .monospacedDigit()
                .foregroundStyle(item.valueColor ?? .primary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .padding(12)
        .background(.cardFill, in: .rect(cornerRadius: Layout.tileRadius))
        .accessibilityElement(children: .combine)
    }
}

#Preview {
    MetricGrid(items: [
        MetricItem(label: "Value", value: "$46,890.69"),
        MetricItem(label: "Cost basis", value: "$40,000.00"),
        MetricItem(label: "Unrealized", value: "+$6,890.69", valueColor: .gain),
    ])
    .padding()
    .background(.screenBackground)
}
