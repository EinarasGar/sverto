import Foundation

nonisolated struct TransactionTypeDefinition: Identifiable, Hashable {
    let key: String
    let label: String
    let description: String
    let systemImage: String
    var id: String { key }
}

nonisolated struct TransactionTypeGroup: Identifiable {
    let title: String
    let types: [TransactionTypeDefinition]
    var id: String { title }
}

nonisolated enum TransactionTypeCatalog {
    static let regular = TransactionTypeDefinition(key: "regular_transaction", label: "Purchase", description: "Buy goods or services", systemImage: "cart")

    static let groups: [TransactionTypeGroup] = [
        TransactionTypeGroup(title: "Everyday", types: [
            regular,
            TransactionTypeDefinition(key: "account_fees", label: "Account Fees", description: "Service or maintenance fees", systemImage: "receipt"),
        ]),
        TransactionTypeGroup(title: "Investments", types: [
            TransactionTypeDefinition(key: "asset_purchase", label: "Buy Asset", description: "Purchase stocks, ETFs, crypto", systemImage: "cart.badge.plus"),
            TransactionTypeDefinition(key: "asset_sale", label: "Sell Asset", description: "Sell holdings for cash", systemImage: "banknote"),
            TransactionTypeDefinition(key: "asset_trade", label: "Trade Assets", description: "Swap one asset for another", systemImage: "arrow.left.arrow.right"),
            TransactionTypeDefinition(key: "cash_dividend", label: "Cash Dividend", description: "Receive dividend in cash", systemImage: "dollarsign.circle"),
            TransactionTypeDefinition(key: "asset_dividend", label: "Asset Dividend", description: "Receive dividend in shares", systemImage: "chart.pie"),
            TransactionTypeDefinition(key: "asset_balance_transfer", label: "Balance Transfer", description: "Move asset balance between accounts", systemImage: "arrow.left.arrow.right.square"),
        ]),
        TransactionTypeGroup(title: "Transfers", types: [
            TransactionTypeDefinition(key: "cash_transfer_in", label: "Cash In", description: "Deposit cash into account", systemImage: "arrow.down.left"),
            TransactionTypeDefinition(key: "cash_transfer_out", label: "Cash Out", description: "Withdraw cash from account", systemImage: "arrow.up.right"),
            TransactionTypeDefinition(key: "cash_balance_transfer", label: "Cash Balance Transfer", description: "Move cash between accounts", systemImage: "arrow.triangle.2.circlepath"),
            TransactionTypeDefinition(key: "asset_transfer_in", label: "Asset Transfer In", description: "Receive assets into account", systemImage: "arrow.down.left.square"),
            TransactionTypeDefinition(key: "asset_transfer_out", label: "Asset Transfer Out", description: "Send assets from account", systemImage: "arrow.up.right.square"),
        ]),
    ]

    static func definition(for key: String) -> TransactionTypeDefinition? {
        groups.lazy.flatMap(\.types).first { $0.key == key }
    }

    static func label(for key: String) -> String {
        (definition(for: key) ?? regular).label
    }
}

nonisolated func apiTypeToConfigKey(_ apiType: String) -> String {
    apiType == TransactionTypeConfig.regularApiType ? TransactionTypeCatalog.regular.key : apiType
}

nonisolated enum AmountSign: Hashable {
    case positive
    case negative
    case any

    // A free-signed amount is usually a purchase, so a new one starts as an expense.
    var blankAmount: String { self == .any ? "-" : "" }
}

nonisolated struct SingleEntryMode: Hashable {
    var amountSign: AmountSign = .any
    var hasCategory = false
    var hasDescription = false
    var hasOriginAsset = false
    var isCash = false
}

nonisolated struct DualEntryMode: Hashable {
    let primaryLabel: String
    let primarySign: AmountSign
    let primaryAmountLabel: String
    let secondaryLabel: String
    let secondarySign: AmountSign
    let secondaryAmountLabel: String
    var sameAccount = false
    var sameAsset = false
    var sharedAmount = false
    var primaryIsCash = false
    var secondaryIsCash = false
}

nonisolated enum EntryMode: Hashable {
    case single(SingleEntryMode)
    case dual(DualEntryMode)

    var primarySign: AmountSign {
        switch self {
        case let .single(mode): mode.amountSign
        case let .dual(mode): mode.primarySign
        }
    }
}

nonisolated struct TransactionTypeConfig: Hashable {
    let key: String
    let entryMode: EntryMode

    static let regularApiType = "regular"

    var apiType: String { key == TransactionTypeCatalog.regular.key ? Self.regularApiType : key }
    var label: String { TransactionTypeCatalog.label(for: key) }

    static let regularTransaction = TransactionTypeConfig(
        key: TransactionTypeCatalog.regular.key,
        entryMode: .single(SingleEntryMode(amountSign: .any, hasCategory: true, hasDescription: true, isCash: true))
    )

    static func forKey(_ typeKey: String) -> TransactionTypeConfig {
        switch typeKey {
        case "account_fees":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .negative, isCash: true)))
        case "cash_transfer_in":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .positive, isCash: true)))
        case "cash_transfer_out":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .negative, isCash: true)))
        case "asset_transfer_in":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .positive)))
        case "asset_transfer_out":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .negative)))
        case "cash_dividend":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .positive, hasOriginAsset: true, isCash: true)))
        case "asset_dividend":
            TransactionTypeConfig(key: typeKey, entryMode: .single(SingleEntryMode(amountSign: .positive)))
        case "asset_purchase":
            TransactionTypeConfig(key: typeKey, entryMode: .dual(DualEntryMode(
                primaryLabel: "Receiving", primarySign: .positive, primaryAmountLabel: "Number of units",
                secondaryLabel: "Paying", secondarySign: .negative, secondaryAmountLabel: "Amount paid",
                sameAccount: true, secondaryIsCash: true
            )))
        case "asset_sale":
            TransactionTypeConfig(key: typeKey, entryMode: .dual(DualEntryMode(
                primaryLabel: "Selling", primarySign: .negative, primaryAmountLabel: "Units sold",
                secondaryLabel: "Receiving", secondarySign: .positive, secondaryAmountLabel: "Proceeds",
                sameAccount: true, secondaryIsCash: true
            )))
        case "asset_trade":
            TransactionTypeConfig(key: typeKey, entryMode: .dual(DualEntryMode(
                primaryLabel: "Paying", primarySign: .negative, primaryAmountLabel: "Units out",
                secondaryLabel: "Receiving", secondarySign: .positive, secondaryAmountLabel: "Units in"
            )))
        case "asset_balance_transfer":
            TransactionTypeConfig(key: typeKey, entryMode: .dual(DualEntryMode(
                primaryLabel: "From", primarySign: .negative, primaryAmountLabel: "Units leaving",
                secondaryLabel: "To", secondarySign: .positive, secondaryAmountLabel: "Units arriving"
            )))
        case "cash_balance_transfer":
            TransactionTypeConfig(key: typeKey, entryMode: .dual(DualEntryMode(
                primaryLabel: "From", primarySign: .negative, primaryAmountLabel: "Amount leaving",
                secondaryLabel: "To", secondarySign: .positive, secondaryAmountLabel: "Amount arriving",
                sameAsset: true, sharedAmount: true, primaryIsCash: true, secondaryIsCash: true
            )))
        default:
            regularTransaction
        }
    }
}
