import Foundation
import SvertoCore

extension Error {
    var friendlyMessage: String {
        if let apiError = self as? ApiError {
            return apiError.friendlyMessage
        }
        if self is CancellationError {
            return "Cancelled"
        }
        return "Something went wrong"
    }
}

extension ApiError {
    var friendlyMessage: String {
        switch self {
        case let .Server(reason, status)
            where [502, 503, 504].contains(status) && (reason.isEmpty || reason == "HTTP \(status)"):
            "The service is temporarily unavailable. Please try again in a moment."
        case let .Network(reason), let .Server(reason, _), let .Timeout(reason), let .Parse(reason):
            reason.isEmpty ? "Something went wrong" : reason
        case .MissingLocalCredential:
            "Your saved session is no longer available. Please sign in again."
        case .AiFeaturesDisabled:
            "AI features are turned off."
        }
    }
}
