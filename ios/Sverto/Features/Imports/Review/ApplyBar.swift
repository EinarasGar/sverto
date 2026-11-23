import SwiftUI

struct ApplyBar: View {
    let state: ApplyBarState
    let applying: Bool
    let busy: Bool
    let onApply: () -> Void
    let onNavigate: (DecisionTarget) -> Void

    private var supporting: String? {
        switch state {
        case let .apply(_, summary, _): summary
        case .navigate: nil
        case let .disabled(text): text
        }
    }

    var body: some View {
        VStack(spacing: 10) {
            if let supporting {
                Text(supporting)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            switch state {
            case let .apply(label, _, _):
                ProminentActionButton(title: applying ? "Applying…" : label, isBusy: applying, action: onApply)
                    .disabled(busy)
            case let .navigate(label, target):
                Button { onNavigate(target) } label: {
                    HStack(spacing: 8) {
                        Text(label)
                            .fontWeight(.semibold)
                            .multilineTextAlignment(.center)
                        Image(systemName: "arrow.down")
                            .fontWeight(.semibold)
                            .accessibilityHidden(true)
                    }
                    .frame(maxWidth: .infinity, minHeight: 28)
                }
                .buttonStyle(.glass)
                .controlSize(.large)
                .disabled(busy)
                .accessibilityHint("Scrolls to the first open decision")
            case .disabled:
                ProminentActionButton(title: "Apply") {}
                    .disabled(true)
            }
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 12)
    }
}

#Preview {
    VStack {
        Spacer()
        ApplyBar(state: .apply(label: "Add 120 transactions", summary: nil, confirm: nil), applying: false, busy: false, onApply: {}, onNavigate: { _ in })
        ApplyBar(state: .navigate(label: "Review 4 possible duplicates", target: .block("x")), applying: false, busy: false, onApply: {}, onNavigate: { _ in })
        ApplyBar(state: .disabled(text: "Answer the open questions first."), applying: false, busy: false, onApply: {}, onNavigate: { _ in })
    }
    .background(.screenBackground)
}
