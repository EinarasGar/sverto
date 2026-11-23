import SvertoCore
import SwiftUI

struct ImportItemsScreen: View {
    let sessionId: String
    @Environment(AppServices.self) private var services

    var body: some View {
        ImportItemsContent(store: services.store, sessionId: sessionId)
    }
}

private struct OpenRow: Identifiable {
    let id: String
}

private struct ImportItemsContent: View {
    @State private var model: ImportItemsViewModel
    @State private var verbFilter: ItemsVerbFilter = .all
    @State private var needsLook = false
    @State private var accountId: String?
    @State private var query = ""
    @State private var openRow: OpenRow?

    init(store: AppStore, sessionId: String) {
        _model = State(initialValue: ImportItemsViewModel(store: store, sessionId: sessionId))
    }

    private var rows: [ImportStagedRow] { model.items?.rows ?? [] }
    private var accounts: [ItemsAccountChoice] { ImportItemsFilters.accountChoices(rows) }
    private var needsLookCount: Int { rows.filter(ImportItemsFilters.needsALook).count }

    private var verbs: [ItemsVerbFilter] {
        ItemsVerbFilter.allCases.filter { filter in
            filter == .all || filter == verbFilter || rows.contains { $0.verb == filter.verb }
        }
    }

    private var isFiltered: Bool { verbFilter != .all || needsLook || accountId != nil }
    private var hasFilters: Bool { verbs.count > 2 || needsLookCount > 0 || accounts.count > 1 }

    private var shown: [ImportStagedRow] {
        rows.filter { row in
            (verbFilter.verb == nil || row.verb == verbFilter.verb)
                && (!needsLook || ImportItemsFilters.needsALook(row))
                && (accountId == nil || row.accountId == accountId)
                && ImportItemsFilters.matches(row, query: query)
        }
    }

    var body: some View {
        content
            .navigationTitle("Proposed transactions")
            .navigationSubtitle(model.items == nil ? "" : ImportCopy.countOf(shown.count, "transaction"))
            .navigationBarTitleDisplayMode(.inline)
            .searchable(text: $query, prompt: "Search description or amount")
            .toolbar {
                if hasFilters {
                    ToolbarItem(placement: .topBarTrailing) { filterMenu }
                }
            }
            .task { await model.load() }
            .toastOnError(model.toast, message: \.text)
            .sheet(item: $openRow) { open in
                if let row = rows.first(where: { $0.id == open.id }) {
                    ProposedRowSheet(
                        row: row,
                        onVerb: { verb in
                            openRow = nil
                            Task { await model.setVerb(itemId: row.id, verb: verb) }
                        },
                        onKeep: { askKey in
                            openRow = nil
                            Task { await model.keep(askKey: askKey) }
                        }
                    )
                }
            }
    }

    @ViewBuilder
    private var content: some View {
        if model.items != nil {
            let visible = shown
            if visible.isEmpty {
                emptyState
            } else {
                List {
                    ForEach(ImportItemsFilters.groupedByDay(visible), id: \.label) { day in
                        Section(day.label) {
                            ForEach(day.rows, id: \.id) { row in
                                Button { openRow = OpenRow(id: row.id) } label: {
                                    ProposedTransactionRow(row: row, showsVerb: verbFilter == .all)
                                }
                                .foregroundStyle(.primary)
                                .swipeActions(edge: .trailing, allowsFullSwipe: true) { trailingSwipe(row) }
                                .swipeActions(edge: .leading) { leadingSwipe(row) }
                            }
                        }
                    }
                }
                .listStyle(.insetGrouped)
                .refreshable { await model.load() }
            }
        } else if let error = model.loadError {
            ContentUnavailableView {
                Label("Couldn't load the transactions", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Try again") {
                    Task { await model.load() }
                }
                .buttonStyle(.borderedProminent)
            }
            .background(.screenBackground)
        } else {
            LoadingScreen()
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if !query.trimmingCharacters(in: .whitespaces).isEmpty {
            ContentUnavailableView.search(text: query)
                .background(.screenBackground)
        } else {
            ContentUnavailableView {
                Label("No transactions match", systemImage: "line.3.horizontal.decrease.circle")
            } actions: {
                if isFiltered {
                    Button("Clear filters", action: clearFilters)
                        .buttonStyle(.bordered)
                }
            }
            .background(.screenBackground)
        }
    }

    private var filterMenu: some View {
        Menu {
            if verbs.count > 2 {
                Section("Show") {
                    Picker("Show", selection: $verbFilter) {
                        ForEach(verbs, id: \.self) { Text($0.label).tag($0) }
                    }
                    .pickerStyle(.inline)
                }
            }
            if needsLookCount > 0 {
                Toggle("Needs a look · \(needsLookCount)", systemImage: "eye", isOn: $needsLook)
            }
            if accounts.count > 1 {
                Picker("Account", systemImage: "building.columns", selection: $accountId) {
                    Text("All accounts").tag(String?.none)
                    ForEach(accounts) { account in
                        Text("\(account.name) · \(account.count)").tag(String?.some(account.id))
                    }
                }
                .pickerStyle(.menu)
            }
            if isFiltered {
                Button("Clear filters", systemImage: "xmark.circle", action: clearFilters)
            }
        } label: {
            Image(systemName: isFiltered ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle")
        }
        .tint(.primary)
        .accessibilityLabel("Filter transactions")
        .accessibilityValue(isFiltered ? "Filtered" : "")
    }

    @ViewBuilder
    private func trailingSwipe(_ row: ImportStagedRow) -> some View {
        if row.verb == .delete {
            if let askKey = row.askKey {
                Button("Keep", systemImage: "arrow.uturn.backward") {
                    Task { await model.keep(askKey: askKey) }
                }
                .tint(.blue)
            }
        } else if row.verb == .skip {
            Button("Add", systemImage: "plus") {
                Task { await model.setVerb(itemId: row.id, verb: .add) }
            }
            .tint(.gain)
        } else {
            Button("Skip", systemImage: "minus") {
                Task { await model.setVerb(itemId: row.id, verb: .skip) }
            }
            .tint(.gray)
        }
    }

    @ViewBuilder
    private func leadingSwipe(_ row: ImportStagedRow) -> some View {
        if row.verbLocked {
            Button("Undo my choice", systemImage: "arrow.uturn.backward") {
                Task { await model.setVerb(itemId: row.id, verb: .auto) }
            }
            .tint(.orange)
        }
    }

    private func clearFilters() {
        verbFilter = .all
        needsLook = false
        accountId = nil
    }
}
