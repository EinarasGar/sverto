import SwiftUI

enum AccountType: Int32, CaseIterable {
    case current = 1
    case savings = 2
    case investment = 3
    case credit = 4
    case personalPension = 5
    case workplacePension = 6
    case mortgage = 7
    case loan = 8
    case realEstate = 9
    case cryptoWallet = 10
    case cash = 11
    case vehicle = 12

    nonisolated static let displayOrder: [AccountType] = [
        .current, .cash, .savings, .investment, .personalPension, .workplacePension,
        .realEstate, .vehicle, .cryptoWallet, .credit, .mortgage, .loan,
    ]

    nonisolated var isInvestment: Bool {
        switch self {
        case .investment, .personalPension, .workplacePension, .cryptoWallet: true
        default: false
        }
    }

    nonisolated var label: String {
        switch self {
        case .current: "Current"
        case .savings: "Savings"
        case .investment: "Investment"
        case .credit: "Credit"
        case .personalPension: "Personal Pension"
        case .workplacePension: "Workplace Pension"
        case .mortgage: "Mortgage"
        case .loan: "Loan"
        case .realEstate: "Real Estate"
        case .cryptoWallet: "Crypto Wallet"
        case .cash: "Cash"
        case .vehicle: "Vehicle"
        }
    }

    nonisolated var systemImage: String {
        switch self {
        case .current: "building.columns"
        case .savings: "dollarsign.bank.building"
        case .investment: "chart.line.uptrend.xyaxis"
        case .credit: "creditcard"
        case .personalPension: "wallet.bifold"
        case .workplacePension: "briefcase"
        case .mortgage: "house"
        case .loan: "dollarsign.square"
        case .realEstate: "building.2"
        case .cryptoWallet: "bitcoinsign.circle"
        case .cash: "banknote"
        case .vehicle: "car"
        }
    }

    nonisolated var tint: Color {
        switch self {
        case .current, .cash: .blue
        case .savings: .teal
        case .investment, .personalPension, .workplacePension, .realEstate, .vehicle, .cryptoWallet: .indigo
        case .credit, .mortgage, .loan: .orange
        }
    }
}

enum AccountTypeVisuals {
    nonisolated static func isInvestment(_ typeId: Int32) -> Bool {
        AccountType(rawValue: typeId)?.isInvestment ?? false
    }

    nonisolated static func label(_ typeId: Int32) -> String {
        AccountType(rawValue: typeId)?.label ?? "Account"
    }

    nonisolated static func systemImage(_ typeId: Int32) -> String {
        AccountType(rawValue: typeId)?.systemImage ?? AccountType.current.systemImage
    }

    nonisolated static func tint(_ typeId: Int32) -> Color {
        AccountType(rawValue: typeId)?.tint ?? AccountType.current.tint
    }

    nonisolated static func orderedTypeIds<S: Sequence>(_ typeIds: S) -> [Int32] where S.Element == Int32 {
        let present = Set(typeIds)
        let known = AccountType.displayOrder.map(\.rawValue).filter(present.contains)
        let unknown = present.subtracting(known).sorted()
        return known + unknown
    }
}
