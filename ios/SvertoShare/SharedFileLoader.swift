import Foundation
import UIKit
import UniformTypeIdentifiers

nonisolated enum SharedFileLoader {
    static func load(_ provider: NSItemProvider, as type: UTType) async -> Data? {
        let contentType = provider.registeredContentTypes.first { $0.conforms(to: type) } ?? type
        if let data = await loadData(provider, as: contentType) { return data }
        if let data = await loadFile(provider, as: contentType) { return data }
        return await loadItem(provider, as: type)
    }

    private static func loadData(_ provider: NSItemProvider, as contentType: UTType) async -> Data? {
        await withCheckedContinuation { continuation in
            _ = provider.loadDataRepresentation(for: contentType) { data, _ in
                continuation.resume(returning: data)
            }
        }
    }

    private static func loadFile(_ provider: NSItemProvider, as contentType: UTType) async -> Data? {
        await withCheckedContinuation { continuation in
            _ = provider.loadFileRepresentation(for: contentType, openInPlace: false) { url, _, _ in
                continuation.resume(returning: url.flatMap(readFile))
            }
        }
    }

    private static func loadItem(_ provider: NSItemProvider, as type: UTType) async -> Data? {
        await withCheckedContinuation { continuation in
            provider.loadItem(forTypeIdentifier: type.identifier) { @Sendable item, _ in
                continuation.resume(returning: itemData(from: item))
            }
        }
    }

    private static func itemData(from item: (any NSSecureCoding)?) -> Data? {
        switch item {
        case let data as Data: data
        case let url as URL: readFile(url)
        case let image as UIImage: image.jpegData(compressionQuality: 0.9)
        default: nil
        }
    }

    private static func readFile(_ url: URL) -> Data? {
        let isScoped = url.startAccessingSecurityScopedResource()
        defer { if isScoped { url.stopAccessingSecurityScopedResource() } }
        return try? Data(contentsOf: url)
    }
}
