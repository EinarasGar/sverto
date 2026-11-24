import Foundation
import ImageIO
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers

nonisolated struct PickedFile: Sendable, Equatable {
    let name: String
    let mimeType: String
    let data: Data
}

nonisolated enum PickResult: Sendable, Equatable {
    case picked(PickedFile)
    case tooLarge(name: String)
    case unreadable(name: String)

    static func failureMessage(_ results: [PickResult]) -> String? {
        let tooLarge = results.compactMap { result in
            if case let .tooLarge(name) = result { "“\(name)”" } else { nil }
        }
        let unreadable = results.compactMap { result in
            if case let .unreadable(name) = result { "“\(name)”" } else { nil }
        }
        let sentences = [
            tooLarge.isEmpty ? nil : "\(tooLarge.joined(separator: ", ")) \(tooLarge.count == 1 ? "is" : "are") over 20 MB",
            unreadable.isEmpty ? nil : "Couldn't open \(unreadable.joined(separator: ", "))",
        ].compactMap { $0 }
        return sentences.isEmpty ? nil : sentences.joined(separator: "\n")
    }
}

struct LocalUpload: Identifiable, Equatable {
    let id: String
    let name: String
    var failed = false
    var error: String?
    var fileId: String?
    var attachedFileId: String?
}

nonisolated enum ImportFileReader {
    static let maxBytes = 20 * 1024 * 1024
    private static let maxNameLength = 255
    private static let unknownMime = "application/octet-stream"
    private static let readableImageTypes: [UTType] = [.jpeg, .png, .gif]
    private static let jpegQuality = 0.9

    @concurrent
    static func read(fileURLs: [URL]) async -> [PickResult] {
        fileURLs.map(read(fileURL:))
    }

    static func read(photoItems: [PhotosPickerItem]) async -> [PickResult] {
        var results: [PickResult] = []
        for (index, item) in photoItems.enumerated() {
            let data = try? await item.loadTransferable(type: Data.self)
            results.append(await prepare(photo: data, number: index + 1))
        }
        return results
    }

    private static func read(fileURL url: URL) -> PickResult {
        let isScoped = url.startAccessingSecurityScopedResource()
        defer {
            if isScoped { url.stopAccessingSecurityScopedResource() }
        }
        let name = safeName(url.lastPathComponent)
        if let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize, size > maxBytes {
            return .tooLarge(name: name)
        }
        guard let data = try? Data(contentsOf: url), !data.isEmpty else { return .unreadable(name: name) }
        guard data.count <= maxBytes else { return .tooLarge(name: name) }
        return .picked(PickedFile(name: name, mimeType: mimeType(forExtension: url.pathExtension), data: data))
    }

    @concurrent
    private static func prepare(photo data: Data?, number: Int) async -> PickResult {
        guard let data, let image = readableImage(data) else { return .unreadable(name: "Photo \(number)") }
        let name = "Photo \(number).\(image.type.preferredFilenameExtension ?? "jpg")"
        guard image.data.count <= maxBytes else { return .tooLarge(name: name) }
        return .picked(PickedFile(name: name, mimeType: image.type.preferredMIMEType ?? "image/jpeg", data: image.data))
    }

    // The AI readers don't understand HEIC and other camera formats, so anything but JPEG, PNG and GIF is re-encoded.
    static func readableImage(_ data: Data) -> (data: Data, type: UTType)? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let typeIdentifier = CGImageSourceGetType(source),
              let type = UTType(typeIdentifier as String) else { return nil }
        if let readable = readableImageTypes.first(where: { type.conforms(to: $0) }) {
            return (data, readable)
        }
        return jpegData(from: source).map { ($0, .jpeg) }
    }

    private static func jpegData(from source: CGImageSource) -> Data? {
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, UTType.jpeg.identifier as CFString, 1, nil) else {
            return nil
        }
        let options: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: jpegQuality]
        CGImageDestinationAddImageFromSource(destination, source, 0, options as CFDictionary)
        guard CGImageDestinationFinalize(destination), output.length > 0 else { return nil }
        return output as Data
    }

    private static func safeName(_ raw: String) -> String {
        let cleaned = raw
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "\\", with: "_")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return String((cleaned.isEmpty ? "statement" : cleaned).prefix(maxNameLength))
    }

    private static func mimeType(forExtension pathExtension: String) -> String {
        guard let mime = UTType(filenameExtension: pathExtension)?.preferredMIMEType,
              mime.filter({ $0 == "/" }).count == 1,
              !mime.contains(";") else { return unknownMime }
        return mime
    }
}

enum ImportFileOrigin {
    case files
    case photos
}
