import Foundation
import Observation
import UniformTypeIdentifiers

@Observable
final class ShareImportModel {
    enum Phase: Equatable {
        case importing
        case finished(added: Int, skipped: Int)
    }

    private(set) var phase: Phase = .importing

    func run(_ items: [NSExtensionItem]) async {
        let providers = items
            .flatMap { $0.attachments ?? [] }
            .filter { $0.hasItemConformingToTypeIdentifier(UTType.image.identifier) || $0.hasItemConformingToTypeIdentifier(UTType.pdf.identifier) }
        var added = 0
        for provider in providers {
            if await importFile(from: provider) {
                added += 1
            }
        }
        phase = .finished(added: added, skipped: providers.count - added)
    }

    private func importFile(from provider: NSItemProvider) async -> Bool {
        if provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            guard let raw = await SharedFileLoader.load(provider, as: .image), raw.count <= SharedFileDropbox.maxBytes else { return false }
            let jpeg = await Task.detached { SharedImageEncoder.jpeg(from: raw) }.value
            guard let jpeg else { return false }
            return SharedFileDropbox.store(jpeg, as: .jpeg)
        }
        guard let pdf = await SharedFileLoader.load(provider, as: .pdf) else { return false }
        return SharedFileDropbox.store(pdf, as: .pdf)
    }
}
