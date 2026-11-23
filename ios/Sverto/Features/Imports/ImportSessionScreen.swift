import PhotosUI
import SvertoCore
import SwiftUI

struct ImportSessionScreen: View {
    let sessionId: String
    var pickFilesOnOpen = false
    @Environment(AppServices.self) private var services

    var body: some View {
        ImportSessionContent(store: services.store, sessionId: sessionId, pickFilesOnOpen: pickFilesOnOpen)
    }
}

private enum ImportSessionSheet: Identifiable {
    case accountPicker(rawAccount: String)
    case assetSearch(AssetAsk)
    case row(id: String)
    case gaps(blockId: String)

    var id: String {
        switch self {
        case let .accountPicker(rawAccount): "account-\(rawAccount)"
        case let .assetSearch(ask): "asset-\(ask.askKey)"
        case let .row(id): "row-\(id)"
        case let .gaps(blockId): "gaps-\(blockId)"
        }
    }
}

private struct ImportSessionContent: View {
    @State private var model: ImportSessionViewModel
    @State private var sheet: ImportSessionSheet?
    @State private var expanded: Set<String> = []
    @State private var scrollTarget: String?
    @State private var confirmsDelete = false
    @State private var confirmsApply = false
    @State private var isFileImporterPresented = false
    @State private var isPhotoPickerPresented = false
    @State private var photoItems: [PhotosPickerItem] = []
    @State private var pickerShown = false
    @State private var isVisible = false
    @Environment(ShellState.self) private var shell
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private let sessionId: String
    private let pickFilesOnOpen: Bool

    init(store: AppStore, sessionId: String, pickFilesOnOpen: Bool) {
        _model = State(initialValue: ImportSessionViewModel(store: store, sessionId: sessionId))
        self.sessionId = sessionId
        self.pickFilesOnOpen = pickFilesOnOpen
    }

    private var applyConfirm: ApplyConfirm? {
        if case let .apply(_, _, confirm) = model.applyBar { confirm } else { nil }
    }

    var body: some View {
        content
            .navigationTitle(model.detail?.summary.title ?? "Import")
            .navigationSubtitle(model.detail.map(ImportSessionBlocks.subtitle) ?? "")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if model.detail != nil {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button("Delete import", systemImage: "trash", role: .destructive) { confirmsDelete = true }
                        } label: {
                            Image(systemName: "ellipsis")
                        }
                        .tint(.primary)
                        .accessibilityLabel("More options")
                        .confirmationDialog("Delete this import?", isPresented: $confirmsDelete, titleVisibility: .visible) {
                            Button("Delete import", role: .destructive, action: delete)
                            Button("Keep import", role: .cancel) {}
                        } message: {
                            Text(deleteMessage)
                        }
                    }
                }
            }
            .onAppear {
                isVisible = true
                model.resume()
            }
            .task { pickFilesIfAsked() }
            .onDisappear {
                isVisible = false
                model.pause()
            }
            .onChange(of: scenePhase) { _, phase in
                guard isVisible else { return }
                if phase == .active {
                    model.resume()
                } else if phase == .background {
                    model.pause()
                }
            }
            .toastOnError(model.toast, message: \.text)
            .aiConsentSheet(model.aiConsent)
            .fileImporter(isPresented: $isFileImporterPresented, allowedContentTypes: [.data], allowsMultipleSelection: true) { result in
                guard case let .success(urls) = result, !urls.isEmpty else { return }
                Task { model.upload(await ImportFileReader.read(fileURLs: urls)) }
            }
            .photosPicker(isPresented: $isPhotoPickerPresented, selection: $photoItems, matching: .images)
            .onChange(of: photoItems) { _, items in
                guard !items.isEmpty else { return }
                photoItems = []
                Task { model.upload(await ImportFileReader.read(photoItems: items)) }
            }
            .sheet(item: $sheet) { sheetContent($0) }
            .alert("Apply this import?", isPresented: $confirmsApply, presenting: applyConfirm) { confirm in
                Button("Keep reviewing", role: .cancel) {}
                Button("Apply", role: confirm.risky ? ButtonRole.destructive : nil) {
                    Task { await model.apply() }
                }
            } message: { confirm in
                Text(([confirm.summary] + confirm.warnings).joined(separator: "\n\n"))
            }
            .sensoryFeedback(.success, trigger: model.applied) { _, applied in applied != nil }
    }

    @ViewBuilder
    private var content: some View {
        if model.detail != nil {
            sessionScroll
        } else if let error = model.loadError {
            ContentUnavailableView {
                Label("Couldn't load this import", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Try again") { model.resume() }
                    .buttonStyle(.borderedProminent)
            }
            .background(.screenBackground)
        } else {
            LoadingScreen()
        }
    }

    private var sessionScroll: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    ForEach(model.blocks) { block in
                        SessionBlockView(block: block, model: model, expanded: $expanded, actions: actions)
                            .id(block.id)
                    }
                }
                .padding(.horizontal, Layout.cardPadding)
                .padding(.top, 8)
                .padding(.bottom, 24)
                .animation(reduceMotion ? nil : .snappy, value: model.blocks.map(\.id))
            }
            .groupedScreenBackground()
            .refreshable { await model.reload() }
            .scrollDismissesKeyboard(.interactively)
            .safeAreaBar(edge: .bottom) {
                if let bar = model.applyBar {
                    ApplyBar(
                        state: bar,
                        applying: model.applying,
                        busy: model.working,
                        onApply: apply,
                        onNavigate: navigate
                    )
                }
            }
            .onChange(of: scrollTarget) { _, target in
                guard let target else { return }
                scrollTarget = nil
                withAnimation(reduceMotion ? nil : .snappy) { proxy.scrollTo(target, anchor: .top) }
            }
        }
    }

    private var actions: SessionActions {
        SessionActions(
            navigate: navigate,
            openRow: { sheet = .row(id: $0) },
            openAllRows: { shell.push(.importItems(sessionId: sessionId)) },
            pickAccount: { sheet = .accountPicker(rawAccount: $0) },
            createAccount: { raw in
                Task {
                    if await model.prepareAccountCreation(rawAccount: raw) { shell.push(.addAccount) }
                }
            },
            searchAsset: { sheet = .assetSearch($0) },
            reviewGaps: { sheet = .gaps(blockId: $0) },
            addFiles: { origin in
                model.aiConsent.perform {
                    switch origin {
                    case .files: isFileImporterPresented = true
                    case .photos: isPhotoPickerPresented = true
                    }
                }
            },
            viewTransactions: viewTransactions
        )
    }

    private var deleteMessage: String {
        let sources = model.detail?.sources ?? []
        let bankAccounts = sources.contains(where: \.isBankAccount)
        let bankOnly = bankAccounts && sources.allSatisfy(\.isBankAccount)
        let linksPending = bankAccounts && model.detail?.summary.status != .completed
        return [
            bankOnly ? "The import is deleted." : "The import and its uploaded files are deleted.",
            linksPending ? "The bank account links it set up are removed too." : nil,
            "Nothing in your ledger changes, including transactions this import already applied.",
        ]
        .compactMap { $0 }
        .joined(separator: " ")
    }

    @ViewBuilder
    private func sheetContent(_ sheet: ImportSessionSheet) -> some View {
        switch sheet {
        case let .accountPicker(rawAccount):
            AccountPickerSheet(accounts: model.accounts ?? [], selectedAccountId: model.accountChoices[rawAccount]) { account in
                model.chooseAccount(rawAccount: rawAccount, accountId: account.id)
            }
        case let .assetSearch(ask):
            AssetPickerSheet(title: "Which asset is “\(ask.raw)”?") { asset in
                model.chooseAsset(askKey: ask.askKey, asset: ImportAssetCandidate(id: asset.id, ticker: asset.ticker, name: asset.name))
            }
        case let .row(id):
            if let row = model.readyItems?.rows.first(where: { $0.id == id }) {
                ProposedRowSheet(
                    row: row,
                    onVerb: { verb in
                        self.sheet = nil
                        Task { await model.setVerb(itemIds: [row.id], verb: verb) }
                    },
                    onKeep: { askKey in
                        self.sheet = nil
                        Task { await model.answerUnlisted(askKey: askKey, delete: false) }
                    }
                )
            } else {
                ContentUnavailableView("This transaction is no longer in the import", systemImage: "tray")
                    .presentationDetents([.medium])
            }
        case let .gaps(blockId):
            if case let .balanceCheck(block)? = model.blocks.first(where: { $0.id == blockId }) {
                BalanceGapsSheet(
                    block: block,
                    enabled: !model.working,
                    onSetAdjustments: { ids, add in
                        Task { await model.setVerb(itemIds: ids, verb: add ? .add : .auto) }
                    },
                    onAnswerUnlisted: { askKey, delete in
                        Task { await model.answerUnlisted(askKey: askKey, delete: delete) }
                    }
                )
            } else {
                ContentUnavailableView("This balance has no open gaps", systemImage: "checkmark.circle")
                    .presentationDetents([.medium])
            }
        }
    }

    private func navigate(_ target: DecisionTarget) {
        switch target {
        case let .row(id): sheet = .row(id: id)
        case let .block(id): scrollTarget = id
        }
    }

    private func apply() {
        if applyConfirm == nil {
            Task { await model.apply() }
        } else {
            confirmsApply = true
        }
    }

    private func delete() {
        let owner = shell.navigationOwner
        Task {
            if await model.delete() { shell.pop(in: owner) }
        }
    }

    private func viewTransactions() {
        shell.dismissSettings()
        shell.setPath([], for: .transactions)
        shell.selectedTab = .transactions
    }

    private func pickFilesIfAsked() {
        guard pickFilesOnOpen, !pickerShown else { return }
        pickerShown = true
        isFileImporterPresented = true
    }
}
