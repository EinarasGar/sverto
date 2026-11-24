import SwiftUI

struct ShareStatusView: View {
    let model: ShareImportModel
    let onDone: () -> Void
    let onCancel: () -> Void

    private let brandTint = Color(red: 0x15 / 255, green: 0xBA / 255, blue: 0x81 / 255)

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: symbolName)
                .font(.system(size: 48))
                .foregroundStyle(.tint)
                .contentTransition(.symbolEffect(.replace))
            Text(title)
                .font(.title2.weight(.semibold))
                .multilineTextAlignment(.center)
            Text(message)
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            if case .importing = model.phase {
                ProgressView()
                    .controlSize(.large)
                Button("Cancel", role: .cancel, action: onCancel)
            } else {
                Button("Done", action: onDone)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
            }
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(.systemGroupedBackground))
        .tint(brandTint)
        .animation(.default, value: model.phase)
    }

    private var symbolName: String {
        switch model.phase {
        case .importing: "doc.viewfinder"
        case let .finished(added, _): added > 0 ? "checkmark.circle.fill" : "exclamationmark.triangle.fill"
        }
    }

    private var title: LocalizedStringKey {
        switch model.phase {
        case .importing: "Adding to Sverto…"
        case let .finished(added, _): added > 0 ? "Added ^[\(added) file](inflect: true)." : "Nothing was added."
        }
    }

    private var message: LocalizedStringKey {
        switch model.phase {
        case .importing:
            "Saving your receipts for review."
        case let .finished(added, skipped):
            if added > 0, skipped == 0 {
                "Open Sverto to review them."
            } else if added > 0 {
                "Open Sverto to review them. ^[\(skipped) file](inflect: true) couldn't be added (20 MB limit)."
            } else {
                "Sverto accepts images and PDFs up to 20 MB each."
            }
        }
    }
}
