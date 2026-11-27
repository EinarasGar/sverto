import SwiftUI
import UIKit

struct QuickUploadCard: View {
    let item: QuickUploadUiItem
    let onTap: () -> Void
    let onRetry: () -> Void
    let onDismiss: () -> Void

    private var isReady: Bool { item.status == .ready }
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 10) {
                QuickUploadStatusBadge(status: item.status)
                HStack(alignment: .top, spacing: 10) {
                    thumbnail
                    VStack(alignment: .leading, spacing: 2) {
                        bodyText
                        if dynamicTypeSize.isAccessibilitySize, isReady {
                            amountLabel
                        }
                    }
                    Spacer(minLength: 8)
                    if !dynamicTypeSize.isAccessibilitySize, isReady {
                        amountLabel
                    }
                }
            }
            .contentShape(.rect)
            .hoverEffect(.highlight)
            .onTapGesture {
                if isReady { onTap() }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(accessibilityDescription)
            .accessibilityAddTraits(isReady ? .isButton : [])
            .accessibilityHint(isReady ? "Opens the proposed transaction" : "")
            footer
        }
        .padding(14)
        .background(Color(.tertiarySystemGroupedBackground), in: .rect(cornerRadius: Layout.tileRadius))
        .onChange(of: item.status) { _, status in
            if status == .ready || status == .failed {
                UIAccessibility.post(notification: .announcement, argument: accessibilityDescription)
            }
        }
    }

    @ViewBuilder
    private var thumbnail: some View {
        if let data = item.thumbnail, let image = UIImage(data: data) {
            Image(uiImage: image)
                .resizable()
                .scaledToFill()
                .frame(width: 44, height: 44)
                .clipShape(.rect(cornerRadius: 10))
                .accessibilityHidden(true)
        } else {
            Image(systemName: "doc.viewfinder")
                .font(.title)
                .foregroundStyle(.secondary)
                .frame(width: 44, height: 44)
                .accessibilityHidden(true)
        }
    }

    @ViewBuilder
    private var bodyText: some View {
        switch item.status {
        case .queued:
            Text("Receipt")
            Text("Queued • Will upload when online")
                .font(.footnote)
                .foregroundStyle(.secondary)
        case .uploading:
            Text("Uploading…")
        case .processing:
            Text("Analyzing…")
            Text("AI is extracting transaction details")
                .font(.footnote)
                .foregroundStyle(.secondary)
        case .ready:
            Text(item.proposalSummary?.description ?? "Receipt")
                .fixedSize(horizontal: false, vertical: true)
            if let date = item.proposalSummary?.date, !date.isEmpty {
                Text(date)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        case .failed:
            Text("Processing failed")
            Text(item.errorMessage ?? "Unknown error")
                .font(.footnote)
                .foregroundStyle(.red)
        }
    }

    @ViewBuilder
    private var footer: some View {
        switch item.status {
        case .uploading:
            ProgressView()
                .controlSize(.small)
                .accessibilityLabel("Uploading")
        case .processing:
            ProgressView()
                .accessibilityLabel("Processing")
        case .failed:
            let layout = dynamicTypeSize.isAccessibilitySize
                ? AnyLayout(VStackLayout(alignment: .trailing, spacing: 8))
                : AnyLayout(HStackLayout(spacing: 8))
            layout {
                if !dynamicTypeSize.isAccessibilitySize {
                    Spacer()
                }
                Button(action: onRetry) {
                    Text("Retry")
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.bordered)
                Button(role: .destructive, action: onDismiss) {
                    Text("Dismiss")
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.borderless)
            }
            .frame(maxWidth: .infinity, alignment: .trailing)
        case .ready:
            HStack {
                Spacer()
                Button(role: .destructive, action: onDismiss) {
                    Text("Discard")
                        .frame(minWidth: 44, minHeight: 44)
                        .contentShape(.rect)
                }
                .buttonStyle(.borderless)
            }
        case .queued:
            EmptyView()
        }
    }

    @ViewBuilder
    private var amountLabel: some View {
        if let amount = readyAmount {
            Text(amount)
                .font(.body)
                .monospacedDigit()
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var readyAmount: String? {
        guard let summary = item.proposalSummary,
              let amountText = summary.amount,
              let value = Double(amountText) else { return nil }
        return TransactionAmountPresentation.format(value, unit: summary.currency ?? "", signed: true)
    }

    private var accessibilityDescription: String {
        switch item.status {
        case .queued: "Quick upload, waiting for connection"
        case .uploading: "Quick upload, uploading"
        case .processing: "Quick upload, processing"
        case .ready:
            ["Quick upload ready to review", item.proposalSummary?.description, item.proposalSummary?.date, readyAmount]
                .compactMap { $0 }
                .filter { !$0.isEmpty }
                .joined(separator: ", ")
        case .failed:
            ["Quick upload failed", item.errorMessage].compactMap { $0 }.joined(separator: ", ")
        }
    }
}

private struct QuickUploadStatusBadge: View {
    let status: QuickUploadStatus

    private var text: String {
        switch status {
        case .queued: "Waiting for connection"
        case .uploading: "Uploading…"
        case .processing: "Reading receipt…"
        case .ready: "Ready to review"
        case .failed: "Failed"
        }
    }

    private var systemImage: String? {
        switch status {
        case .queued: "icloud.slash"
        case .failed: "exclamationmark.triangle"
        case .uploading, .processing, .ready: nil
        }
    }

    private var tint: Color {
        switch status {
        case .queued: .secondary
        case .uploading, .processing: Color(.systemBlue)
        case .ready: .brand
        case .failed: .red
        }
    }

    var body: some View {
        HStack(spacing: 4) {
            if let systemImage {
                Image(systemName: systemImage)
                    .font(.caption2.weight(.semibold))
                    .accessibilityHidden(true)
            }
            Text(text)
                .font(.caption.weight(.medium))
        }
        .foregroundStyle(tint)
        .padding(.horizontal, 10)
        .padding(.vertical, 4)
        .background(tint.opacity(0.14), in: .capsule)
    }
}
