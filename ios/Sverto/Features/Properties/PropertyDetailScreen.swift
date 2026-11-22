import SvertoCore
import SwiftUI

struct PropertyDetailScreen: View {
    let propertyId: String
    let name: String

    @Environment(AppServices.self) private var services

    var body: some View {
        PropertyDetailContent(store: services.store, propertyId: propertyId, name: name)
    }
}

private enum PropertySheet: Identifiable {
    case addValuation
    case edit
    case linkLoan

    var id: Self { self }
}

private struct PropertyDetailContent: View {
    @State private var model: PropertyDetailViewModel
    @State private var sheet: PropertySheet?
    @State private var loanToUnlink: AccountListItem?
    @Environment(ShellState.self) private var shell
    @Environment(ToastCenter.self) private var toasts

    private let name: String

    init(store: AppStore, propertyId: String, name: String) {
        self.name = name
        _model = State(initialValue: PropertyDetailViewModel(store: store, propertyId: propertyId))
    }

    var body: some View {
        content
            .navigationTitle(model.detail?.account.name ?? name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    menu
                        .tint(.primary)
                        .disabled(model.detail == nil)
                }
            }
            .sheet(item: $sheet) { sheet in
                sheetContent(sheet)
            }
            .alert(
                "Unlink loan?",
                isPresented: Binding(get: { loanToUnlink != nil }, set: { if !$0 { loanToUnlink = nil } }),
                presenting: loanToUnlink
            ) { loan in
                Button("Unlink", role: .destructive) {
                    Task { await unlink(loan) }
                }
                Button("Cancel", role: .cancel) {}
            } message: { loan in
                Text("Are you sure you want to unlink \(loan.name) from \(model.detail?.account.name ?? name)? The loan stays a normal account and shows in your accounts list again.")
            }
            .task { await model.appear() }
    }

    private var menu: some View {
        Menu {
            Button("Add valuation", systemImage: "plus") { sheet = .addValuation }
                .disabled(model.currencyTicker == nil)
            Button("Valuations", systemImage: "clock.arrow.circlepath", action: showValuations)
                .disabled(model.currencyTicker == nil)
            Button("Edit", systemImage: "pencil") { sheet = .edit }
            Button("Link loan", systemImage: "link") { sheet = .linkLoan }
        } label: {
            Image(systemName: "ellipsis")
        }
        .accessibilityLabel("More options")
    }

    @ViewBuilder
    private var content: some View {
        if let detail = model.detail {
            ScrollView {
                VStack(alignment: .leading, spacing: 24) {
                    PropertyHeader(account: detail.account, isJoint: model.isJoint)
                    value(detail)
                    figures(detail)
                    loans(detail)
                }
                .padding(.horizontal, Layout.cardPadding)
                .padding(.vertical, 8)
            }
            .groupedScreenBackground()
            .refreshable { await model.refresh() }
        } else if let error = model.loadError {
            ContentUnavailableView {
                Label("Couldn't load property", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") {
                    Task { await model.retry() }
                }
                .buttonStyle(.borderedProminent)
            }
            .background(.screenBackground)
        } else {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.screenBackground)
        }
    }

    private func value(_ detail: PropertyDetail) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            valueChart(detail)
            ValuationButtons(
                isEnabled: model.currencyTicker != nil,
                onAdd: { sheet = .addValuation },
                onShowValuations: showValuations
            )
            .frame(maxWidth: .infinity)
            if let error = model.currencyError {
                HStack(alignment: .firstTextBaseline) {
                    Text("Couldn't load the property's currency, so its value can't be updated. \(error)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    Button("Retry") {
                        Task { await model.retryCurrency() }
                    }
                    .font(.footnote)
                }
            }
        }
    }

    @ViewBuilder
    private func valueChart(_ detail: PropertyDetail) -> some View {
        if let chart = model.valueChart {
            ChartView(
                periods: [ChartPeriodData(period: ChartPeriod.all.rawValue, points: chart.line)],
                baseTicker: detail.baseTicker,
                selectedPeriod: .constant(.all),
                showsPeriodPicker: false,
                changeSuffix: "since purchase",
                markers: chart.valuations
            )
        } else if let error = model.valueChartError {
            ContentUnavailableView {
                Label("Couldn't load value history", systemImage: "exclamationmark.triangle")
            } description: {
                Text(error)
            } actions: {
                Button("Retry") {
                    Task { await model.retryValueChart() }
                }
                .buttonStyle(.bordered)
            }
        } else {
            ChartSkeleton(showsPeriodPicker: false)
        }
    }

    private func figures(_ detail: PropertyDetail) -> some View {
        let summary = detail.summary
        let ticker = detail.baseTicker
        var items = [
            MetricItem(label: "Value", value: Money.format(summary.value ?? 0, ticker: ticker)),
            MetricItem(label: "Loan left", value: Money.format(summary.loanBalance ?? 0, ticker: ticker)),
            MetricItem(label: "Equity", value: Money.format(summary.equity ?? 0, ticker: ticker)),
            MetricItem(label: "Loan-to-value", value: summary.loanToValue.map { PercentText.format($0, signed: false) } ?? "—"),
        ]
        if model.isJoint {
            items.append(MetricItem(label: "Your share of equity", value: Money.format(summary.equityShare ?? 0, ticker: ticker)))
        }
        return VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Summary")
            MetricGrid(items: items)
        }
    }

    private func loans(_ detail: PropertyDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            SectionHeader("Loans", trailing: {
                AnyView(
                    Button("Link loan", systemImage: "link") { sheet = .linkLoan }
                        .font(.subheadline)
                )
            })
            GroupedCard {
                if detail.loans.isEmpty {
                    Text("No linked loans")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .padding(Layout.cardPadding)
                } else {
                    VStack(spacing: 0) {
                        ForEach(Array(detail.loans.enumerated()), id: \.element.id) { index, loan in
                            PropertyLoanRow(
                                loan: loan,
                                baseTicker: detail.baseTicker,
                                isUnlinking: model.unlinkingLoanId == loan.id,
                                onView: {
                                    shell.push(.accountDetail(
                                        accountId: loan.id,
                                        accountName: loan.name,
                                        accountTypeId: loan.accountTypeId
                                    ))
                                },
                                onUnlink: { loanToUnlink = loan }
                            )
                            if index < detail.loans.count - 1 {
                                RowDivider(leadingInset: Layout.cardPadding + 36 + Layout.rowSpacing)
                            }
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func sheetContent(_ sheet: PropertySheet) -> some View {
        switch sheet {
        case .addValuation:
            if let currencyTicker = model.currencyTicker {
                ValuationSheet(title: "Add valuation", currencyTicker: currencyTicker) { value, date in
                    try await model.addValuation(value, on: date)
                }
            }
        case .edit:
            if let detail = model.detail {
                EditPropertySheet(propertyId: model.propertyId, account: detail.account, summary: detail.summary) {
                    await model.reloadWithChart()
                }
            }
        case .linkLoan:
            LinkLoanSheet { accountId in
                try await model.linkLoan(accountId: accountId)
            }
        }
    }

    private func showValuations() {
        guard let summary = model.summary, let currencyTicker = model.currencyTicker else { return }
        shell.push(.propertyValuations(
            assetId: summary.assetId,
            currencyAssetId: summary.currencyAssetId,
            currencyTicker: currencyTicker
        ))
    }

    private func unlink(_ loan: AccountListItem) async {
        if let error = await model.unlinkLoan(accountId: loan.id) {
            toasts.show(error)
        }
    }
}

private struct ValuationButtons: View {
    let isEnabled: Bool
    let onAdd: () -> Void
    let onShowValuations: () -> Void

    var body: some View {
        HStack(spacing: 0) {
            Button(action: onAdd) {
                Label("Add valuation", systemImage: "plus")
                    .padding(.leading, 16)
                    .padding(.trailing, 12)
                    .frame(minHeight: 44)
                    .contentShape(.rect)
            }
            Rectangle()
                .fill(Color.onAccent.opacity(0.4))
                .frame(width: 1, height: 22)
                .accessibilityHidden(true)
            Button(action: onShowValuations) {
                Image(systemName: "clock.arrow.circlepath")
                    .padding(.leading, 12)
                    .padding(.trailing, 16)
                    .frame(minHeight: 44)
                    .contentShape(.rect)
            }
            .accessibilityLabel("Valuations")
        }
        .font(.body.weight(.semibold))
        .foregroundStyle(Color.onAccent)
        .background(Color.brand, in: .capsule)
        .buttonStyle(.plain)
        .disabled(!isEnabled)
        .opacity(isEnabled ? 1 : 0.5)
    }
}

private struct PropertyHeader: View {
    let account: AccountListItem
    let isJoint: Bool

    var body: some View {
        HStack(spacing: 14) {
            AccountTypeBadge(typeId: account.accountTypeId)
            VStack(alignment: .leading, spacing: 4) {
                Text(account.name)
                    .font(.title3.weight(.semibold))
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: 8) {
                    Text(AccountTypeVisuals.label(account.accountTypeId))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    if isJoint {
                        Text("Joint · \(PercentText.format(account.ownershipShare, signed: false, fractionDigits: 0))")
                            .font(.caption.weight(.medium))
                            .foregroundStyle(.tint)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color.brand.opacity(Layout.iconTileOpacity), in: .capsule)
                    }
                }
            }
        }
        .accessibilityElement(children: .combine)
    }
}

private struct PropertyLoanRow: View {
    let loan: AccountListItem
    let baseTicker: String
    let isUnlinking: Bool
    let onView: () -> Void
    let onUnlink: () -> Void

    private var subtitle: String {
        let type = AccountTypeVisuals.label(loan.accountTypeId)
        guard loan.ownershipShare < 1 else { return type }
        return "\(type) · \(PercentText.format(loan.ownershipShare, signed: false, fractionDigits: 0)) share"
    }

    var body: some View {
        HStack(spacing: 4) {
            Button(action: onView) {
                HStack(spacing: Layout.rowSpacing) {
                    AccountTypeBadge(typeId: loan.accountTypeId, size: 36)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(loan.name)
                            .font(.body.weight(.medium))
                            .foregroundStyle(Color.primary)
                            .fixedSize(horizontal: false, vertical: true)
                        Text(subtitle)
                            .font(.subheadline)
                            .foregroundStyle(Color.secondary)
                    }
                    Spacer(minLength: 8)
                    Text(Money.format(loan.balance ?? 0, ticker: baseTicker))
                        .font(.body.weight(.semibold))
                        .monospacedDigit()
                        .foregroundStyle(Color.primary)
                        .lineLimit(1)
                        .fixedSize()
                }
                .padding(.vertical, Layout.rowSpacing)
                .padding(.leading, Layout.cardPadding)
                .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityElement(children: .combine)
            .accessibilityHint("Opens the loan's account")
            Group {
                if isUnlinking {
                    ProgressView()
                } else {
                    Button(action: onUnlink) {
                        LucideIcon(name: "unlink", size: 20, color: .red)
                            .frame(width: 44, height: 44)
                            .contentShape(.rect)
                    }
                    .buttonStyle(.borderless)
                    .accessibilityLabel("Unlink \(loan.name)")
                }
            }
            .frame(width: 44, height: 44)
            .padding(.trailing, 4)
        }
    }
}
