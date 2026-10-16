import SvertoCore
import SwiftUI

struct CategoryTypePickerList: View {
    let types: [ManagedCategoryType]
    @Binding var selection: Int32?

    @State private var query = ""
    @Environment(\.dismiss) private var dismiss

    private var filteredTypes: [ManagedCategoryType] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return types }
        return types.filter { $0.name.localizedCaseInsensitiveContains(trimmed) }
    }

    var body: some View {
        List(filteredTypes, id: \.id) { type in
            Button {
                selection = type.id
                dismiss()
            } label: {
                HStack {
                    Text(type.name)
                        .foregroundStyle(Color.primary)
                    Spacer()
                    if type.id == selection {
                        Image(systemName: "checkmark")
                            .foregroundStyle(.tint)
                    }
                }
                .contentShape(.rect)
            }
            .accessibilityAddTraits(type.id == selection ? .isSelected : [])
        }
        .overlay {
            if filteredTypes.isEmpty {
                ContentUnavailableView.search(text: query)
            }
        }
        .searchable(text: $query, placement: .navigationBarDrawer(displayMode: .always), prompt: "Search types…")
        .navigationTitle("Type")
        .navigationBarTitleDisplayMode(.inline)
    }
}
