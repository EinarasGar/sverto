import Foundation
import ImageIO
import UIKit
import UniformTypeIdentifiers

nonisolated enum QuickUploadPrep {
    private static let cameraQuality = 0.9
    private static let thumbnailWidth: Double = 200
    private static let thumbnailQuality: Double = 0.6
    // The AI provider rejects images over 5 MB, so larger ones are downscaled.
    private static let maxImageBytes = 5 * 1024 * 1024
    private static let maxDownscaledEdge = 3072
    private static let downscaledQuality: Double = 0.85

    @concurrent
    static func read(cameraImage: UIImage) async -> PickResult {
        guard let data = cameraImage.jpegData(compressionQuality: cameraQuality) else { return .unreadable(name: "Camera photo") }
        return .picked(PickedFile(name: "Camera photo.jpg", mimeType: "image/jpeg", data: data))
    }

    @concurrent
    static func prepare(_ file: PickedFile) async -> PreparedQuickUpload? {
        if file.mimeType == "application/pdf" || file.mimeType.hasPrefix("text/") {
            return PreparedQuickUpload(data: file.data, thumbnail: Data(), mimeType: file.mimeType)
        }
        guard var image = ImportFileReader.readableImage(file.data) else { return nil }
        if image.data.count > maxImageBytes {
            guard let downscaled = downscaledJpeg(image.data) else { return nil }
            image = (downscaled, .jpeg)
        }
        guard let thumbnail = thumbnail(from: image.data) else { return nil }
        return PreparedQuickUpload(data: image.data, thumbnail: thumbnail, mimeType: image.type.preferredMIMEType ?? "image/jpeg")
    }

    private static func downscaledJpeg(_ imageData: Data) -> Data? {
        guard let source = CGImageSourceCreateWithData(imageData as CFData, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceThumbnailMaxPixelSize: maxDownscaledEdge,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        return jpegData(image, quality: downscaledQuality)
    }

    private static func thumbnail(from imageData: Data) -> Data? {
        guard let source = CGImageSourceCreateWithData(imageData as CFData, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let pixelWidth = properties[kCGImagePropertyPixelWidth] as? Double, pixelWidth > 0,
              let pixelHeight = properties[kCGImagePropertyPixelHeight] as? Double, pixelHeight > 0 else { return nil }
        let orientation = properties[kCGImagePropertyOrientation] as? UInt32 ?? 1
        let isRotated = orientation >= 5
        let displayWidth = isRotated ? pixelHeight : pixelWidth
        let displayHeight = isRotated ? pixelWidth : pixelHeight
        let maxPixelSize = Int((thumbnailWidth * max(displayWidth, displayHeight) / displayWidth).rounded(.up))
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        return jpegData(image, quality: thumbnailQuality)
    }

    private static func jpegData(_ image: CGImage, quality: Double) -> Data? {
        let output = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(output, UTType.jpeg.identifier as CFString, 1, nil) else {
            return nil
        }
        let options: [CFString: Any] = [kCGImageDestinationLossyCompressionQuality: quality]
        CGImageDestinationAddImage(destination, image, options as CFDictionary)
        guard CGImageDestinationFinalize(destination), output.length > 0 else { return nil }
        return output as Data
    }
}
