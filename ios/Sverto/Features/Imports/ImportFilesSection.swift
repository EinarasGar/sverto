import SvertoCore
import SwiftUI

struct ImportFilesSection: View {
    let sources: [ImportSource]
    let uploads: [LocalUpload]
    let acceptsFiles: Bool
    let enabled: Bool
    let collapsible: Bool
    @Binding var isExpanded: Bool
    let onAddFiles: (ImportFileOrigin) -> Void
    let onRetryFile: (String) -> Void
    let onRemoveFile: (String) -> Void
    let onDateOrder: (String, ImportDateOrder) -> Void
    let onRetryUpload: (String) -> Void
    let onDismissUpload: (String) -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var hasBankAccounts: Bool { sources.contains(where: \.isBankAccount) }
    private var hasFiles: Bool { !uploads.isEmpty || sources.contains { $0.file != nil } }
    private var isEmpty: Bool { sources.isEmpty && uploads.isEmpty }

    private func title(files: String) -> String {
        if !hasBankAccounts { return files }
        return hasFiles ? "Sources" : "Bank accounts"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            if collapsible {
                collapsed
            } else {
                if !isEmpty {
                    SectionHeader(title(files: "Files"), trailing: {
                        AnyView(CountLabel(ImportCopy.sourcesCount(sources, uploads: uploads.count)))
                    })
                    GroupedCard {
                        rows
                    }
                    .clipShape(.rect(cornerRadius: Layout.cardRadius))
                }
                if acceptsFiles {
                    AddFilesMenu(prominent: isEmpty, onAdd: onAddFiles)
                    if isEmpty {
                        Text("Include every account you move money between, so transfers import as one transaction. CSV, Excel, PDF, OFX and other statements up to 20 MB.")
                            .font(.footnote)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
            }
        }
    }

    private var collapsed: some View {
        GroupedCard {
            VStack(spacing: 0) {
                HStack(spacing: 4) {
                    Button {
                        withAnimation(reduceMotion ? nil : .snappy) { isExpanded.toggle() }
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: hasFiles ? "doc.on.doc" : "building.columns")
                                .foregroundStyle(.secondary)
                                .frame(width: 24)
                                .accessibilityHidden(true)
                            Text("\(title(files: "Source files")) · \(sources.count + uploads.count)")
                                .frame(maxWidth: .infinity, alignment: .leading)
                            Image(systemName: "chevron.down")
                                .font(.footnote.weight(.semibold))
                                .foregroundStyle(.secondary)
                                .rotationEffect(.degrees(isExpanded ? 180 : 0))
                                .accessibilityHidden(true)
                        }
                        .padding(.leading, Layout.cardPadding)
                        .padding(.trailing, acceptsFiles ? 4 : Layout.cardPadding)
                        .frame(minHeight: 52)
                        .contentShape(.rect)
                    }
                    .buttonStyle(.cardRow)
                    .accessibilityValue(isExpanded ? "Expanded" : "Collapsed")
                    if acceptsFiles {
                        AddFilesMenu(prominent: false, compact: true, onAdd: onAddFiles)
                            .padding(.trailing, 8)
                    }
                }
                if isExpanded {
                    RowDivider()
                    rows
                }
            }
        }
        .clipShape(.rect(cornerRadius: Layout.cardRadius))
    }

    private var rows: some View {
        VStack(spacing: 0) {
            ForEach(Array(sources.enumerated()), id: \.element.id) { index, source in
                if index > 0 {
                    RowDivider(leadingInset: Layout.cardPadding + Layout.rowGlyphSize + Layout.rowSpacing)
                }
                if let file = source.file {
                    FileRow(
                        file: file,
                        recordedTransactions: source.recordedTransactions,
                        acceptsFiles: acceptsFiles,
                        enabled: enabled,
                        onRetry: { onRetryFile(file.fileId) },
                        onRemove: { onRemoveFile(file.fileId) },
                        onDateOrder: { onDateOrder(file.fileId, $0) }
                    )
                } else if let bank = source.bankAccount {
                    BankAccountRow(account: bank, recordedTransactions: source.recordedTransactions)
                }
            }
            ForEach(Array(uploads.enumerated()), id: \.element.id) { index, upload in
                if !sources.isEmpty || index > 0 {
                    RowDivider(leadingInset: Layout.cardPadding + Layout.rowGlyphSize + Layout.rowSpacing)
                }
                UploadRow(
                    upload: upload,
                    onRetry: { onRetryUpload(upload.id) },
                    onDismiss: { onDismissUpload(upload.id) }
                )
            }
        }
    }
}

struct AddFilesMenu: View {
    let prominent: Bool
    var compact = false
    let onAdd: (ImportFileOrigin) -> Void

    var body: some View {
        Menu {
            Button("Choose Files", systemImage: "folder") { onAdd(.files) }
            Button("Photo Library", systemImage: "photo.on.rectangle") { onAdd(.photos) }
        } label: {
            if compact {
                Label("Add", systemImage: "plus")
                    .font(.subheadline.weight(.semibold))
                    .padding(.horizontal, 8)
                    .frame(minHeight: 44)
                    .contentShape(.rect)
            } else {
                Label("Add files", systemImage: "arrow.up.doc")
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity, minHeight: 28)
            }
        }
        .modifier(AddFilesMenuStyle(prominent: prominent, compact: compact))
        .accessibilityLabel("Add files")
    }
}

private struct AddFilesMenuStyle: ViewModifier {
    let prominent: Bool
    let compact: Bool

    @ViewBuilder
    func body(content: Content) -> some View {
        if compact {
            content
        } else if prominent {
            content.buttonStyle(.borderedProminent).controlSize(.large)
        } else {
            content.buttonStyle(.bordered).controlSize(.large)
        }
    }
}

private struct SourceRowLayout<Trailing: View>: View {
    let systemImage: String
    let name: String
    let status: String
    var statusIsError = false
    var note: String? = nil
    var loading = false
    var action: AnyView? = nil
    @ViewBuilder let trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .center, spacing: Layout.rowSpacing) {
            IconTile(tint: .secondary) { _ in
                if loading {
                    ProgressView()
                } else {
                    Image(systemName: systemImage)
                }
            }
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text(status)
                    .font(.subheadline)
                    .foregroundStyle(statusIsError ? Color.loss : Color.secondary)
                    .lineLimit(3)
                if let note {
                    Text(note)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                }
                if let action {
                    action
                        .padding(.top, 6)
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing()
        }
        .padding(.leading, Layout.cardPadding)
        .padding(.trailing, 8)
        .padding(.vertical, 12)
    }
}

private struct FileRow: View {
    let file: ImportFile
    let recordedTransactions: Int64
    let acceptsFiles: Bool
    let enabled: Bool
    let onRetry: () -> Void
    let onRemove: () -> Void
    let onDateOrder: (ImportDateOrder) -> Void

    @State private var confirmsRemove = false

    private var failed: Bool { file.status == .failed }
    private var dateOrder: ImportDateOrder? { file.status == .parsed ? file.dateOrder : nil }

    var body: some View {
        SourceRowLayout(
            systemImage: Self.glyph(file.name),
            name: file.name,
            status: status,
            statusIsError: failed,
            note: dateOrder.map(Self.dateOrderNote),
            loading: file.pausedUntil == nil && file.status == .processing,
            action: failed && acceptsFiles ? AnyView(retryButton) : nil
        ) {
            if acceptsFiles {
                Menu {
                    if let dateOrder {
                        let other: ImportDateOrder = dateOrder == .dayFirst ? .monthFirst : .dayFirst
                        Button(other == .monthFirst ? "Read dates month first" : "Read dates day first", systemImage: "calendar") {
                            onDateOrder(other)
                        }
                    }
                    Button("Remove", systemImage: "trash", role: .destructive) { confirmsRemove = true }
                } label: {
                    Image(systemName: "ellipsis.circle")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                        .frame(width: 44, height: 44)
                        .contentShape(.rect)
                }
                .disabled(!enabled)
                .accessibilityLabel("Options for \(file.name)")
                .confirmationDialog("Remove this file?", isPresented: $confirmsRemove, titleVisibility: .visible) {
                    Button("Remove", role: .destructive, action: onRemove)
                    Button("Cancel", role: .cancel) {}
                } message: {
                    Text("“\(file.name)” and its transactions leave this import. Nothing in your ledger changes.")
                }
            }
        }
        .accessibilityElement(children: .contain)
    }

    private var retryButton: some View {
        Button("Retry", systemImage: "arrow.clockwise", action: onRetry)
            .buttonStyle(.bordered)
            .controlSize(.small)
            .disabled(!enabled)
            .accessibilityLabel("Retry reading \(file.name)")
    }

    private var status: String {
        if let pausedUntil = file.pausedUntil {
            return "Paused until \(ImportDates.time(pausedUntil))"
        }
        switch file.status {
        case .pending: return "Waiting to be read"
        case .processing: return "Reading…"
        case .parsed:
            return recordedTransactions > 0 ? "Read · \(ImportCopy.countOf(recordedTransactions, "transaction")) recorded" : "Read"
        case .failed:
            return ["Couldn't be read", file.error].compactMap { $0 }.joined(separator: " · ")
        }
    }

    private static func dateOrderNote(_ order: ImportDateOrder) -> String {
        switch order {
        case .dayFirst: "Dates read day first, so 03/11 is 3 November"
        case .monthFirst: "Dates read month first, so 03/11 is 11 March"
        }
    }

    static func glyph(_ name: String) -> String {
        switch (name as NSString).pathExtension.lowercased() {
        case "csv", "tsv", "xls", "xlsx", "ods", "numbers": "tablecells"
        case "pdf": "doc.richtext"
        case "png", "jpg", "jpeg", "heic", "webp", "gif": "photo"
        default: "doc.text"
        }
    }
}

private struct BankAccountRow: View {
    let account: BankAccountSource
    let recordedTransactions: Int64

    var body: some View {
        SourceRowLayout(
            systemImage: "building.columns",
            name: account.label,
            status: [account.currency, account.accountNumberLast4.map { "••\($0)" }, account.providerName]
                .compactMap { $0 }
                .joined(separator: " · "),
            note: recordedTransactions > 0 ? "\(ImportCopy.countOf(recordedTransactions, "transaction")) recorded" : nil
        ) {
            EmptyView()
        }
        .accessibilityElement(children: .combine)
    }
}

private struct UploadRow: View {
    let upload: LocalUpload
    let onRetry: () -> Void
    let onDismiss: () -> Void

    var body: some View {
        SourceRowLayout(
            systemImage: FileRow.glyph(upload.name),
            name: upload.name,
            status: upload.failed ? "Upload failed · \(upload.error ?? "Try again")" : "Uploading…",
            statusIsError: upload.failed,
            loading: !upload.failed
        ) {
            if upload.failed {
                HStack(spacing: 0) {
                    Button("Retry upload", systemImage: "arrow.clockwise", action: onRetry)
                    Button("Remove upload", systemImage: "xmark", action: onDismiss)
                }
                .labelStyle(.iconOnly)
                .buttonStyle(.borderless)
                .frame(minHeight: 44)
            }
        }
    }
}
