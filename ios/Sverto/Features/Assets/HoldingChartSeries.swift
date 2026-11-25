enum HoldingChartSeries: CaseIterable {
    case price
    case value
    case units

    var label: String {
        switch self {
        case .price: "Price"
        case .value: "Value"
        case .units: "Units"
        }
    }

    var systemImage: String {
        switch self {
        case .price: "chart.line.uptrend.xyaxis"
        case .value: "wallet.bifold"
        case .units: "square.stack.3d.up"
        }
    }

    var caption: String {
        switch self {
        case .price: "Price per unit"
        case .value: "Value of your holdings"
        case .units: "Units you hold"
        }
    }

    // Holding value moves with purchases and sales, so a percentage change would read as a return.
    var valueFormat: ChartValueFormat {
        switch self {
        case .price: .money(showsChangePercent: true)
        case .value: .money(showsChangePercent: false)
        case .units: .units
        }
    }
}
