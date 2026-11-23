import SwiftUI

struct ImportStatusHeader: View {
    let state: StatusHeaderState
    var retryLabel: String? = nil
    var onRetry: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(spacing: 8) {
            ZStack {
                Circle()
                    .fill(state.tone.fill)
                if state.working {
                    ProgressView()
                        .controlSize(.large)
                        .tint(state.tone.tint)
                        .accessibilityLabel(state.label)
                } else {
                    Image(systemName: state.systemImage)
                        .font(.title.weight(.medium))
                        .foregroundStyle(state.tone.tint)
                        .transition(.scale(scale: 0.8).combined(with: .opacity))
                        .accessibilityHidden(true)
                }
            }
            .frame(width: 72, height: 72)
            .animation(reduceMotion ? nil : .snappy, value: state)
            .padding(.bottom, 8)
            Text(state.label)
                .font(.title3.weight(.semibold))
                .accessibilityAddTraits(.isHeader)
            Text(state.summary)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            if let error = state.error {
                Text(error)
                    .font(.subheadline)
                    .foregroundStyle(.loss)
            }
            if let retryLabel {
                Button(retryLabel, systemImage: "arrow.clockwise", action: onRetry)
                    .buttonStyle(.bordered)
                    .padding(.top, 8)
            }
        }
        .multilineTextAlignment(.center)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 24)
        .padding(.vertical, 16)
    }
}

struct ImportReceipt: View {
    let lines: [String]
    let onViewTransactions: () -> Void

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: "checkmark")
                .font(.title.weight(.semibold))
                .foregroundStyle(.gain)
                .frame(width: 72, height: 72)
                .background(Color.gain.opacity(Layout.iconTileOpacity), in: .circle)
                .padding(.bottom, 8)
                .accessibilityHidden(true)
            Text("Import applied")
                .font(.title3.weight(.semibold))
                .accessibilityAddTraits(.isHeader)
            ForEach(lines, id: \.self) { line in
                Text(line)
                    .foregroundStyle(.secondary)
            }
            Button("View in Transactions", action: onViewTransactions)
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .padding(.top, 12)
        }
        .multilineTextAlignment(.center)
        .fixedSize(horizontal: false, vertical: true)
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 24)
        .padding(.vertical, 16)
    }
}

#Preview {
    ScrollView {
        ImportStatusHeader(state: StatusHeaderState(
            label: "Reading files",
            summary: "Sverto is reading the files. It keeps going in the background, so it is safe to leave this page.",
            tone: .brand,
            systemImage: "hourglass",
            working: true
        ))
        ImportStatusHeader(
            state: StatusHeaderState(
                label: "Couldn't categorize",
                summary: "The transactions could not be categorized.",
                tone: .negative,
                systemImage: "exclamationmark.circle",
                working: false,
                error: "Rate limited"
            ),
            retryLabel: "Retry categorizing"
        )
        ImportReceipt(lines: ["120 added", "3 skipped as already recorded"]) {}
    }
    .background(.screenBackground)
}
