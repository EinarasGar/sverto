import SvertoCore
import SwiftUI

struct CustomCategoriesScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        CustomCategoriesContent(store: services.store)
    }
}

private enum CategoriesTab: String, CaseIterable, Identifiable {
    case categories = "Categories"
    case types = "Types"

    var id: Self { self }
}

private struct CategoryFormTarget: Identifiable {
    let category: ManagedCategory?
    var id: Int32 { category?.id ?? -1 }
}

private struct TypeNameTarget: Identifiable {
    let typeId: Int32?
    let name: String
    var id: Int32 { typeId ?? -1 }
}

private struct CustomCategoriesContent: View {
    @State private var model: CustomCategoriesViewModel
    @State private var tab: CategoriesTab = .categories
    @State private var categoryForm: CategoryFormTarget?
    @State private var typeName: TypeNameTarget?
    @State private var categoryToDelete: ManagedCategory?
    @State private var typeToDelete: ManagedCategoryType?
    @State private var completedActions = 0
    @Environment(ToastCenter.self) private var toasts
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(store: AppStore) {
        _model = State(initialValue: CustomCategoriesViewModel(store: store))
    }

    var body: some View {
        VStack(spacing: 0) {
            Picker("View", selection: $tab) {
                ForEach(CategoriesTab.allCases) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .sensoryFeedback(.selection, trigger: tab)
            .padding(.horizontal)
            .padding(.top, 8)
            switch tab {
            case .categories: categoriesList
            case .types: typesList
            }
        }
        .background(.screenBackground)
        .navigationTitle("Custom Categories")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Add", systemImage: "plus", action: add)
                    .tint(.primary)
            }
        }
        .task { await model.start() }
        .onDisappear { model.stop() }
        .sheet(item: $categoryForm) { target in
            CategoryFormSheet(existing: target.category, types: model.state.types) { result in
                try await submitCategory(result)
            }
        }
        .sheet(item: $typeName) { target in
            TypeNameSheet(initialName: target.name, isEdit: target.typeId != nil) { name in
                try await submitType(id: target.typeId, name: name)
            }
        }
        .confirmationDialog(
            "Delete category",
            isPresented: isPresented($categoryToDelete),
            titleVisibility: .visible,
            presenting: categoryToDelete
        ) { category in
            Button("Delete", role: .destructive) {
                Task { await perform { try await model.deleteCategory(id: category.id) } }
            }
            Button("Cancel", role: .cancel) {}
        } message: { category in
            Text("Delete \"\(category.name)\"? This can't be undone.")
        }
        .confirmationDialog(
            "Delete type",
            isPresented: isPresented($typeToDelete),
            titleVisibility: .visible,
            presenting: typeToDelete
        ) { type in
            Button("Delete", role: .destructive) {
                Task { await perform { try await model.deleteType(id: type.id) } }
            }
            Button("Cancel", role: .cancel) {}
        } message: { type in
            Text("Delete \"\(type.name)\"? Categories using it must be removed first.")
        }
        .sensoryFeedback(.success, trigger: completedActions)
    }

    private var categoriesList: some View {
        ScrollViewReader { proxy in
            List {
                ForEach(model.state.categories, id: \.id) { category in
                    CategoryRow(
                        category: category,
                        onEdit: { categoryForm = CategoryFormTarget(category: $0) },
                        onDelete: { categoryToDelete = $0 }
                    )
                }
            }
            .listStyle(.insetGrouped)
            .animation(reduceMotion ? nil : .snappy, value: model.state)
            .overlay {
                listStatus(
                    isEmpty: model.state.categories.isEmpty,
                    emptyTitle: "No categories yet",
                    loadFailedTitle: "Couldn't load categories",
                    systemImage: "tag"
                )
            }
            .refreshable { await model.refresh() }
            .onChange(of: model.state.categories.count) { previous, current in
                guard previous > 0, current > previous, let first = model.state.categories.first else { return }
                withAnimation(reduceMotion ? nil : .default) { proxy.scrollTo(first.id, anchor: .top) }
            }
        }
    }

    private var typesList: some View {
        ScrollViewReader { proxy in
            List {
                ForEach(model.state.types, id: \.id) { type in
                    CategoryTypeRow(
                        type: type,
                        onEdit: { typeName = TypeNameTarget(typeId: $0.id, name: $0.name) },
                        onDelete: { typeToDelete = $0 }
                    )
                }
            }
            .listStyle(.insetGrouped)
            .animation(reduceMotion ? nil : .snappy, value: model.state)
            .overlay {
                listStatus(
                    isEmpty: model.state.types.isEmpty,
                    emptyTitle: "No types yet",
                    loadFailedTitle: "Couldn't load types",
                    systemImage: "rectangle.3.group"
                )
            }
            .refreshable { await model.refresh() }
            .onChange(of: model.state.types.count) { previous, current in
                guard previous > 0, current > previous, let first = model.state.types.first else { return }
                withAnimation(reduceMotion ? nil : .default) { proxy.scrollTo(first.id, anchor: .top) }
            }
        }
    }

    @ViewBuilder
    private func listStatus(isEmpty: Bool, emptyTitle: String, loadFailedTitle: String, systemImage: String) -> some View {
        if isEmpty, model.state.isLoading {
            ProgressView()
                .controlSize(.large)
                .accessibilityLabel("Loading")
        } else if isEmpty, model.state.error != nil {
            ContentUnavailableView(loadFailedTitle, systemImage: "wifi.exclamationmark", description: Text("Pull down to try again."))
                .allowsHitTesting(false)
        } else if isEmpty {
            ContentUnavailableView(emptyTitle, systemImage: systemImage)
                .allowsHitTesting(false)
        }
    }

    private func add() {
        switch tab {
        case .categories: categoryForm = CategoryFormTarget(category: nil)
        case .types: typeName = TypeNameTarget(typeId: nil, name: "")
        }
    }

    private func submitCategory(_ result: CategoryFormResult) async throws {
        if let id = result.id {
            try await model.updateCategory(id: id, name: result.name, icon: result.icon, typeId: result.typeId)
        } else {
            try await model.createCategory(name: result.name, icon: result.icon, typeId: result.typeId)
        }
        completedActions += 1
    }

    private func submitType(id: Int32?, name: String) async throws {
        if let id {
            try await model.updateType(id: id, name: name)
        } else {
            try await model.createType(name: name)
        }
        completedActions += 1
    }

    private func perform(_ action: () async throws -> Void) async {
        do {
            try await action()
            completedActions += 1
        } catch {
            toasts.showError(error.friendlyMessage)
        }
    }

    private func isPresented<Value>(_ value: Binding<Value?>) -> Binding<Bool> {
        Binding(
            get: { value.wrappedValue != nil },
            set: { if !$0 { value.wrappedValue = nil } }
        )
    }
}
