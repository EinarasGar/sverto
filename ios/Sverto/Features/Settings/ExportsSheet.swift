import SvertoCore
import SwiftUI

struct ExportsSheet: View {
    private static let formats: [ExportFormat] = [.csv, .beancount]

    @State private var model: ExportsViewModel
    @State private var format: ExportFormat = .csv
    @Environment(\.dismiss) private var dismiss

    init(store: AppStore) {
        _model = State(initialValue: ExportsViewModel(store: store))
    }

    var body: some View {
        NavigationStack {
            List {
                Section {
                    VStack(spacing: 12) {
                        Picker("Format", selection: $format) {
                            ForEach(Self.formats, id: \.self) { option in
                                Text(option.label).tag(option)
                            }
                        }
                        .pickerStyle(.segmented)
                        .sensoryFeedback(.selection, trigger: format)
                        Button {
                            Task { await model.createExport(format) }
                        } label: {
                            Text("Create export")
                                .frame(maxWidth: .infinity)
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.large)
                    }
                    .disabled(model.isCreating)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                    if model.isCreating {
                        HStack(spacing: 12) {
                            ProgressView()
                            Text("Creating export…").foregroundStyle(.secondary)
                        }
                    }
                    if let error = model.errorMessage {
                        Text(error).foregroundStyle(.red)
                    }
                }
                Section("Previous exports") {
                    if model.isLoading && model.exports.isEmpty {
                        ProgressView().frame(maxWidth: .infinity)
                    } else if model.exports.isEmpty {
                        ContentUnavailableView("No exports yet", systemImage: "square.and.arrow.down")
                    } else {
                        ForEach(model.exports, id: \.id) { export in
                            ExportRow(export: export, isDownloading: model.downloadingId == export.id) {
                                Task { await model.download(export) }
                            }
                            .disabled(model.downloadingId != nil && model.downloadingId != export.id)
                        }
                    }
                }
            }
            .navigationTitle("Export ledger")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .task { await model.load() }
            .sheet(item: $model.downloadedExport) { downloaded in
                ShareSheet(items: [downloaded.url])
                    .presentationDetents([.medium, .large])
            }
            .sensoryFeedback(.success, trigger: model.downloadedExport)
        }
        .presentationDetents([.medium, .large])
    }
}

private struct ExportRow: View {
    let export: LedgerExport
    let isDownloading: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                SettingsIcon(systemImage: "doc.text")
                VStack(alignment: .leading, spacing: 2) {
                    Text(export.format.label)
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .monospacedDigit()
                }
                Spacer()
                if isDownloading {
                    ProgressView()
                } else {
                    Image(systemName: "square.and.arrow.up")
                        .foregroundStyle(.secondary)
                        .accessibilityHidden(true)
                }
            }
        }
        .foregroundStyle(.primary)
        .accessibilityLabel("\(export.format.label) export, \(subtitle)")
        .accessibilityHint("Downloads and shares the file")
    }

    private var subtitle: String {
        let date = export.createdDate?.formatted(date: .abbreviated, time: .shortened) ?? export.createdAt
        let size = export.sizeBytes.formatted(.byteCount(style: .file))
        return "\(date) · \(size)"
    }
}
