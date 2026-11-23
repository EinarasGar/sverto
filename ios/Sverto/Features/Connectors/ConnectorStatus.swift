import Foundation
import SvertoCore

enum ConnectorStatus: String {
    case pending
    case pendingOauth = "pending_oauth"
    case active
    case paused
    case error
    case revoked
}

extension ConnectorConnection {
    var connectionStatus: ConnectorStatus? { ConnectorStatus(rawValue: status) }
}

extension ConnectorBinding {
    var bindingStatus: ConnectorStatus? { ConnectorStatus(rawValue: status) }

    var isPaused: Bool { bindingStatus == .paused }

    var isWaitingForImport: Bool { bindingStatus == .pending }
}

extension BindingStatus {
    var connectorStatus: ConnectorStatus {
        switch self {
        case .active: .active
        case .paused: .paused
        }
    }
}

extension ProviderAccountHistory {
    var label: String? {
        switch self {
        case .fetching: "Fetching history…"
        case .failed: "Couldn't fetch history"
        case .notFetched, .fetched: nil
        }
    }
}
