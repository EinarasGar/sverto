import SwiftUI

struct QuickUploadsSection: View {
    private let viewModel: QuickUploadViewModel
    private let onItemTap: (QuickUploadUiItem) -> Void
    @State private var pendingDismissal: QuickUploadUiItem?
    @State private var showsDismissConfirmation = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(viewModel: QuickUploadViewModel, onItemTap: @escaping (QuickUploadUiItem) -> Void) {
        self.viewModel = viewModel
        self.onItemTap = onItemTap
    }

    var body: some View {
        if !viewModel.items.isEmpty {
            GroupedCard {
                VStack(alignment: .leading, spacing: 12) {
                    header
                    ForEach(viewModel.items) { item in
                        QuickUploadCard(
                            item: item,
                            onTap: { onItemTap(item) },
                            onRetry: { viewModel.retry(id: item.id) },
                            onDismiss: {
                                pendingDismissal = item
                                showsDismissConfirmation = true
                            }
                        )
                        .modifier(dismissDialog(.trigger, for: item))
                    }
                }
                .padding(Layout.cardPadding)
            }
            .animation(reduceMotion ? nil : .snappy, value: viewModel.items)
            .modifier(dismissDialog(.screen))
        }
    }

    private func dismissDialog(_ source: DialogSource, for card: QuickUploadUiItem? = nil) -> some ViewModifier {
        ConfirmationDialog(
            source: source,
            title: pendingDismissal?.status == .failed ? "Dismiss receipt?" : "Discard receipt?",
            isPresented: Binding(
                get: { showsDismissConfirmation && (card == nil || pendingDismissal?.id == card?.id) },
                set: { showsDismissConfirmation = $0 }
            ),
            item: pendingDismissal
        ) { item in
            Button(item.status == .failed ? "Dismiss" : "Discard", role: .destructive) {
                pendingDismissal = nil
                Task { await viewModel.dismiss(id: item.id) }
            }
            Button("Keep Receipt", role: .cancel) { pendingDismissal = nil }
        } message: { _ in
            Text("This removes the receipt from Quick Uploads.")
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "clock")
                .font(.body.weight(.semibold))
                .foregroundStyle(.brand)
                .frame(width: 34, height: 34)
                .background(Color.brand.opacity(0.14), in: .circle)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: 2) {
                Text("Quick Uploads (\(viewModel.items.count))")
                    .font(.subheadline.weight(.semibold))
                Text("Receipts awaiting review")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
    }
}
