import Foundation

nonisolated enum SharedFileInbox {
    @concurrent
    static func drain() async -> [PickResult] {
        guard let folder = folderURL(),
              let files = try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil, options: .skipsHiddenFiles)
        else { return [] }
        let results = await ImportFileReader.read(fileURLs: files.sorted { $0.lastPathComponent < $1.lastPathComponent })
        for file in files {
            try? FileManager.default.removeItem(at: file)
        }
        return results
    }

    private static func folderURL() -> URL? {
        guard let groupId = Bundle.main.object(forInfoDictionaryKey: "SvertoAppGroupIdentifier") as? String, !groupId.isEmpty,
              let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: groupId) else { return nil }
        return container.appendingPathComponent("shared-files", isDirectory: true)
    }
}
