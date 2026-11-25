import SvertoCore
import SwiftUI

struct PropertyValuationsScreen: View {
    let assetId: Int32
    let currencyAssetId: Int32
    let currencyTicker: String

    @Environment(AppServices.self) private var services

    var body: some View {
        PropertyValuationsContent(
            store: services.store,
            assetId: assetId,
            currencyAssetId: currencyAssetId,
            currencyTicker: currencyTicker
        )
    }
}

private enum ValuationSheetMode: Identifiable {
    case add
    case edit(ChartPoint)

    var id: String {
        switch self {
        case .add: "add"
        case let .edit(valuation): "edit-\(valuation.timestamp)"
        }
    }
}

private struct PropertyValuationsContent: View {
    @State private var model: PropertyValuationsViewModel
    @State private var sheet: ValuationSheetMode?
    @State private var valuationToDelete: ChartPoint?
    @Environment(ToastCenter.self) private var toasts

    init(store: AppStore, assetId: Int32, currencyAssetId: Int32, currencyTicker: String) {
        _model = State(initialValue: PropertyValuationsViewModel(
            store: store,
            assetId: assetId,
            currencyAssetId: currencyAssetId,
            currencyTicker: currencyTicker
        ))
    }

    var body: some View {
        content
            .navigationTitle("Valuations")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        sheet = .add
                    } label: {
                        Image(systemName: "plus")
                    }
                    .tint(.primary)
                    .accessibilityLabel("Add valuation")
                }
            }
            .sheet(item: $sheet) { mode in
                switch mode {
                case .add:
                    ValuationSheet(title: "Add valuation", currencyTicker: model.currencyTicker) { value, date in
                        try await model.add(value: value, on: date)
                    }
                case let .edit(valuation):
                    ValuationSheet(title: "Edit valuation", currencyTicker: model.currencyTicker, valuation: valuation) { value, date in
                        try await model.edit(valuation, value: value, on: date)
                    }
                }
            }
            .alert(
                "Delete valuation?",
                isPresented: Binding(get: { valuationToDelete != nil }, set: { if !$0 { valuationToDelete = nil } }),
                presenting: valuationToDelete
            ) { valuation in
                Button("Delete", role: .destructive) {
                    Task { await delete(valuation) }
                }
                Button("Cancel", role: .cancel) {}
            } message: { valuation in
                Text(model.deleteMessage(for: valuation))
            }
            .task { await model.start() }
    }

    @ViewBuilder
    private var content: some View {
        if model.isLoading {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        } else if let error = model.loadError, model.valuations.isEmpty {
            ContentUnavailableView {
                Label("Couldn't load valuations", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") { Task { await model.load() } }
            }
            .background(.screenBackground)
        } else if model.valuations.isEmpty {
            ContentUnavailableView(
                "No valuations yet",
                systemImage: "clock.arrow.circlepath",
                description: Text("Tap + to add what the property is worth.")
            )
            .background(.screenBackground)
        } else {
            List(model.valuations, id: \.timestamp) { valuation in
                HStack(spacing: 4) {
                    Button {
                        sheet = .edit(valuation)
                    } label: {
                        HStack {
                            Text(DateText.mediumDate(epochSeconds: valuation.timestamp))
                                .foregroundStyle(Color.primary)
                            Spacer(minLength: 8)
                            Text(Money.format(valuation.value, ticker: model.currencyTicker))
                                .monospacedDigit()
                                .foregroundStyle(Color.primary)
                        }
                        .contentShape(.rect)
                    }
                    .buttonStyle(.plain)
                    .accessibilityHint("Edits this valuation")
                    Button {
                        valuationToDelete = valuation
                    } label: {
                        Image(systemName: "trash")
                            .foregroundStyle(.red)
                            .frame(width: 44, height: 44)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Delete valuation from \(DateText.mediumDate(epochSeconds: valuation.timestamp))")
                }
                .swipeActions(edge: .trailing) {
                    Button("Delete", systemImage: "trash", role: .destructive) {
                        valuationToDelete = valuation
                    }
                }
            }
            .listStyle(.insetGrouped)
            .refreshable { await model.refresh() }
        }
    }

    private func delete(_ valuation: ChartPoint) async {
        if let error = await model.delete(valuation) {
            toasts.showError(error)
        }
    }
}
