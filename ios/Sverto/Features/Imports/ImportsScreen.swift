import SvertoCore
import SwiftUI

struct ImportsScreen: View {
    @Environment(AppServices.self) private var services

    var body: some View {
        ImportsContent(store: services.store)
    }
}

private struct ImportsContent: View {
    @State private var model: ImportsViewModel
    @Environment(ShellState.self) private var shell

    init(store: AppStore) {
        _model = State(initialValue: ImportsViewModel(store: store))
    }

    var body: some View {
        content
            .navigationTitle("Imports")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if model.sessions?.isEmpty == false {
                    ToolbarItem(placement: .primaryAction) {
                        Button("New import", systemImage: "plus", action: startImport)
                            .tint(.primary)
                            .disabled(model.creating)
                    }
                }
            }
            .task { await model.load() }
            .toastOnError(model.toast, message: \.text)
            .aiConsentSheet(model.aiConsent)
    }

    @ViewBuilder
    private var content: some View {
        if let sessions = model.sessions {
            if sessions.isEmpty {
                EmptyImports(creating: model.creating, onStart: startImport)
            } else {
                List {
                    section("In progress", sessions: model.inProgress, showsStatus: true)
                    section("Finished", sessions: model.finished, showsStatus: false)
                }
                .listStyle(.insetGrouped)
                .refreshable { await model.load() }
            }
        } else if let error = model.loadError {
            ContentUnavailableView {
                Label("Couldn't load imports", systemImage: "exclamationmark.triangle")
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
    private func section(_ title: String, sessions: [ImportSessionSummary], showsStatus: Bool) -> some View {
        if !sessions.isEmpty {
            Section(title) {
                ForEach(sessions, id: \.id) { session in
                    ImportSessionRow(session: session, showsStatus: showsStatus) {
                        shell.push(.importSession(id: session.id))
                    }
                }
            }
        }
    }

    private func startImport() {
        model.aiConsent.perform {
            Task {
                await model.create { id in shell.push(.importSession(id: id, pickFiles: true)) }
            }
        }
    }
}

private struct ImportSessionRow: View {
    let session: ImportSessionSummary
    let showsStatus: Bool
    let action: () -> Void
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    private var copy: SessionStatusCopy { session.status.copy }

    private var started: String {
        let relative = RelativeTime.string(epochSeconds: session.createdAt)
        let isRecent = Date().timeIntervalSince(DateText.date(fromUnix: session.createdAt)) < 7 * 24 * 60 * 60
        return "Started \(isRecent ? relative.lowercasedFirst : relative)"
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: Layout.rowSpacing) {
                IconTile(systemImage: glyph, tint: copy.tone == .ghost ? .secondary : copy.tone.tint, shape: .circle)
                VStack(alignment: .leading, spacing: 4) {
                    Text(session.title)
                        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 2)
                    Text(started)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    if showsStatus {
                        ToneChip(label: copy.label, tone: copy.tone)
                    }
                }
                .fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 8)
                Image(systemName: "chevron.forward")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(.tertiary)
                    .accessibilityHidden(true)
            }
            .padding(.vertical, 2)
        }
        .foregroundStyle(.primary)
        .accessibilityElement(children: .combine)
        .accessibilityLabel([session.title, started, showsStatus ? copy.label : nil].compactMap { $0 }.joined(separator: ", "))
    }

    private var glyph: String {
        switch session.status {
        case .collecting: "doc.badge.plus"
        case .parsing, .reconciling: "hourglass"
        case .needsInput: "questionmark.bubble"
        case .proposalReady: "checklist"
        case .completed: "checkmark"
        }
    }
}

private struct EmptyImports: View {
    let creating: Bool
    let onStart: () -> Void

    private static let steps = ["Add statements", "Answer a few questions", "Review and apply"]

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                ContentUnavailableView {
                    Label("Import your statements", systemImage: "arrow.up.doc")
                } description: {
                    Text("Bring in transactions from bank and broker exports. Nothing is written to your ledger until you approve it.")
                }
                GroupedCard {
                    VStack(alignment: .leading, spacing: 16) {
                        ForEach(Array(Self.steps.enumerated()), id: \.offset) { index, step in
                            HStack(spacing: 14) {
                                Text("\(index + 1)")
                                    .font(.subheadline.weight(.semibold))
                                    .foregroundStyle(.tint)
                                    .frame(width: 32, height: 32)
                                    .background(Color.brand.opacity(Layout.iconTileOpacity), in: .circle)
                                    .accessibilityHidden(true)
                                Text(step)
                                    .accessibilityLabel("Step \(index + 1), \(step)")
                            }
                        }
                    }
                    .padding(Layout.cardPadding + 4)
                }
                ProminentActionButton(title: "Start an import", isBusy: creating, action: onStart)
                    .disabled(creating)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 16)
            .frame(maxWidth: Layout.readableWidth)
            .frame(maxWidth: .infinity)
        }
        .background(.screenBackground)
    }
}

private extension String {
    var lowercasedFirst: String {
        prefix(1).lowercased() + dropFirst()
    }
}
