import Foundation
import UniformTypeIdentifiers

nonisolated enum SharedFileDropbox {
    static let maxBytes = 20 * 1024 * 1024

    static func store(_ data: Data, as type: UTType) -> Bool {
        guard data.count <= maxBytes, let folder = folderURL() else { return false }
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            let file = folder.appendingPathComponent(UUID().uuidString).appendingPathExtension(for: type)
            try data.write(to: file, options: .atomic)
            return true
        } catch {
            return false
        }
    }

    private static func folderURL() -> URL? {
        guard let groupId = Bundle.main.object(forInfoDictionaryKey: "SvertoAppGroupIdentifier") as? String, !groupId.isEmpty,
              let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: groupId) else { return nil }
        return container.appendingPathComponent("shared-files", isDirectory: true)
    }
}
