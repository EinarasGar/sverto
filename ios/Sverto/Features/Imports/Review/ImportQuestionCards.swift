import SwiftUI

struct QuestionOption: Identifiable, Hashable {
    let id: String
    let title: String
    var subtitle: String? = nil
    var note: String? = nil
}

struct QuestionCard<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        GroupedCard {
            VStack(alignment: .leading, spacing: 0) {
                content
            }
            .padding(.vertical, 8)
        }
        .clipShape(.rect(cornerRadius: Layout.cardRadius))
    }
}

struct QuestionTitle: View {
    let title: String
    var supporting: String? = nil
    var trailing: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .firstTextBaseline) {
                Text(title)
                    .font(.headline)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityAddTraits(.isHeader)
                if let trailing {
                    CountLabel(trailing)
                }
            }
            if let supporting {
                Text(supporting)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .truncationMode(.middle)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
        .padding(.horizontal, Layout.cardPadding)
        .padding(.vertical, 8)
    }
}

struct OptionList: View {
    let options: [QuestionOption]
    let selectedId: String?
    let enabled: Bool
    let onSelect: (String) -> Void

    var body: some View {
        VStack(spacing: 0) {
            ForEach(options) { option in
                RowDivider()
                Button { onSelect(option.id) } label: {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(option.title)
                            if let subtitle = option.subtitle {
                                Text(subtitle)
                                    .font(.subheadline)
                                    .foregroundStyle(.secondary)
                            }
                            if let note = option.note {
                                Text(note)
                                    .font(.footnote)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .fixedSize(horizontal: false, vertical: true)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Image(systemName: "checkmark")
                            .font(.body.weight(.semibold))
                            .foregroundStyle(.tint)
                            .opacity(option.id == selectedId ? 1 : 0)
                            .accessibilityHidden(true)
                    }
                    .padding(.horizontal, Layout.cardPadding)
                    .padding(.vertical, 10)
                    .frame(minHeight: 50)
                    .contentShape(.rect)
                }
                .buttonStyle(.cardRow)
                .disabled(!enabled)
                .accessibilityAddTraits(option.id == selectedId ? .isSelected : [])
            }
        }
    }
}

struct ExtraOptionRow: View {
    let systemImage: String
    let label: String
    let enabled: Bool
    let action: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            RowDivider()
            Button(action: action) {
                Label(label, systemImage: systemImage)
                    .foregroundStyle(.tint)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, Layout.cardPadding)
                    .frame(minHeight: 50)
                    .contentShape(.rect)
            }
            .buttonStyle(.cardRow)
            .disabled(!enabled)
        }
    }
}

struct CardActions<Content: View>: View {
    private let content: Content
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        let layout = dynamicTypeSize.isAccessibilitySize
            ? AnyLayout(VStackLayout(alignment: .trailing, spacing: 8))
            : AnyLayout(HStackLayout(spacing: 8))
        layout {
            content
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
    }
}

struct AccountQuestionCard: View {
    let rawAccount: String
    let sourceNames: [String]
    let options: [QuestionOption]
    let selectedId: String?
    let canExclude: Bool
    let sameAccount: Bool?
    let enabled: Bool
    let onSelect: (String) -> Void
    let onChooseAnother: () -> Void
    let onCreateAccount: () -> Void
    let onSameAccountChange: (Bool) -> Void
    let onExclude: () -> Void
    let onLink: () -> Void

    var body: some View {
        QuestionCard {
            QuestionTitle(
                title: "Which account is “\(rawAccount)”?",
                supporting: sourceNames.isEmpty ? nil : "From \(sourceNames.joined(separator: ", "))"
            )
            OptionList(options: options, selectedId: selectedId, enabled: enabled, onSelect: onSelect)
            ExtraOptionRow(systemImage: "magnifyingglass", label: "Choose another account…", enabled: enabled, action: onChooseAnother)
            ExtraOptionRow(systemImage: "plus", label: "Create a new account", enabled: enabled, action: onCreateAccount)
            if let sameAccount {
                RowDivider()
                Toggle(isOn: Binding(get: { sameAccount }, set: { onSameAccountChange($0) })) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("These are the same account")
                        Text(sameAccount ? "Your answer covers every file" : "Answer one file at a time")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                }
                .disabled(!enabled)
                .padding(.horizontal, Layout.cardPadding)
                .padding(.vertical, 10)
            }
            CardActions {
                if canExclude {
                    Button("Skip this account", action: onExclude)
                        .buttonStyle(.bordered)
                        .accessibilityLabel("Skip account “\(rawAccount)”")
                }
                Button("Link", action: onLink)
                    .buttonStyle(.borderedProminent)
                    .disabled(selectedId == nil)
                    .accessibilityLabel("Link “\(rawAccount)” to the selected account")
            }
            .disabled(!enabled)
            .padding(.horizontal, Layout.cardPadding)
            .padding(.top, 12)
            .padding(.bottom, 8)
        }
    }
}

struct AssetQuestionCard: View {
    let raw: String
    let options: [QuestionOption]
    let selectedId: String?
    let canExclude: Bool
    let enabled: Bool
    let onSelect: (String) -> Void
    let onSearch: () -> Void
    let onLeaveOut: () -> Void
    let onLink: () -> Void

    @State private var confirmsLeaveOut = false

    var body: some View {
        QuestionCard {
            QuestionTitle(title: "Which asset is “\(raw)”?")
            OptionList(options: options, selectedId: selectedId, enabled: enabled, onSelect: onSelect)
            ExtraOptionRow(systemImage: "magnifyingglass", label: "Search assets…", enabled: enabled, action: onSearch)
            CardActions {
                if canExclude {
                    Button("Leave out") { confirmsLeaveOut = true }
                        .buttonStyle(.bordered)
                        .confirmationDialog("Leave out “\(raw)”?", isPresented: $confirmsLeaveOut, titleVisibility: .visible) {
                            Button("Leave out", role: .destructive, action: onLeaveOut)
                            Button("Cancel", role: .cancel) {}
                        } message: {
                            Text("Rows that use this asset won't be imported.")
                        }
                }
                Button("Link", action: onLink)
                    .buttonStyle(.borderedProminent)
                    .disabled(selectedId == nil)
                    .accessibilityLabel("Link “\(raw)” to the selected asset")
            }
            .disabled(!enabled)
            .padding(.horizontal, Layout.cardPadding)
            .padding(.top, 12)
            .padding(.bottom, 8)
        }
    }
}

struct BalanceQuestionCard: View {
    let accountName: String
    let ticker: String
    let asOfDate: Int64
    let enabled: Bool
    let onCheck: (String) -> Void

    private var title: String {
        [accountName, ticker].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    var body: some View {
        ReviewCard {
            LabelWithChip {
                Text(title)
                    .font(.headline)
            } chip: {
                ToneChip(label: "Not checked", tone: .ghost)
            }
            DetailLine("No statement gives a balance to check against. Your bank or broker app shows the balance after each transaction.")
            Text("What was the balance on \(ImportDates.date(asOfDate))?")
                .fixedSize(horizontal: false, vertical: true)
            BalanceEntryField(suffix: ticker, enabled: enabled, onSubmit: onCheck)
        }
    }
}

struct BalanceEntryField: View {
    let suffix: String
    let enabled: Bool
    let onSubmit: (String) -> Void

    @State private var text = ""
    @FocusState private var isFocused: Bool

    private var quantity: String? { Self.quantity(from: text, locale: .current) }

    static func quantity(from raw: String, locale: Locale) -> String? {
        let formatter = NumberFormatter()
        formatter.locale = locale
        let grouping = formatter.groupingSeparator ?? ","
        let decimal = formatter.decimalSeparator ?? "."
        var text = raw.trimmingCharacters(in: .whitespaces).replacingOccurrences(of: formatter.minusSign, with: "-")
        if grouping.unicodeScalars.allSatisfy({ CharacterSet.whitespaces.contains($0) }) {
            text = text.replacingOccurrences(of: " ", with: grouping)
        }
        let group = NSRegularExpression.escapedPattern(for: grouping)
        let point = NSRegularExpression.escapedPattern(for: decimal)
        guard let pattern = try? Regex("-?([0-9]+|[0-9]{1,3}(\(group)[0-9]{3})+)(\(point)[0-9]+)?"),
              text.wholeMatch(of: pattern) != nil else { return nil }
        return text.replacingOccurrences(of: grouping, with: "").replacingOccurrences(of: decimal, with: ".")
    }

    var body: some View {
        HStack(spacing: 12) {
            HStack(spacing: 6) {
                TextField("Balance", text: $text)
                    .foregroundStyle(text.isEmpty || quantity != nil ? Color.primary : Color.loss)
                    .keyboardType(.numbersAndPunctuation)
                    .submitLabel(.done)
                    .focused($isFocused)
                    .monospacedDigit()
                    .onSubmit(submit)
                if !suffix.isEmpty {
                    Text(suffix)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(.horizontal, 12)
            .frame(minHeight: 44)
            .background(Color(.tertiarySystemFill), in: .rect(cornerRadius: 10))
            Button("Check", action: submit)
                .buttonStyle(.borderedProminent)
                .disabled(quantity == nil)
        }
        .disabled(!enabled)
    }

    private func submit() {
        guard let quantity else { return }
        isFocused = false
        onSubmit(quantity)
    }
}
