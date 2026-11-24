import Foundation
import ImageIO
import Testing
import UIKit
import UniformTypeIdentifiers
@testable import Sverto

private func pngData(width: Int, height: Int) throws -> Data {
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    let renderer = UIGraphicsImageRenderer(size: CGSize(width: width, height: height), format: format)
    let image = renderer.image { context in
        UIColor.systemTeal.setFill()
        context.fill(CGRect(x: 0, y: 0, width: width, height: height))
        UIColor.white.setFill()
        context.fill(CGRect(x: width / 4, y: height / 4, width: width / 2, height: height / 2))
    }
    return try #require(image.pngData())
}

private func isJpeg(_ data: Data) -> Bool {
    data.count > 3 && data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF
}

private func heicData(from png: Data) throws -> Data {
    let source = try #require(CGImageSourceCreateWithData(png as CFData, nil))
    let output = NSMutableData()
    let destination = try #require(CGImageDestinationCreateWithData(output, UTType.heic.identifier as CFString, 1, nil))
    CGImageDestinationAddImageFromSource(destination, source, 0, nil)
    try #require(CGImageDestinationFinalize(destination))
    return output as Data
}

private func prepare(_ data: Data, mimeType: String) async -> PreparedQuickUpload? {
    await QuickUploadPrep.prepare(PickedFile(name: "receipt", mimeType: mimeType, data: data))
}

struct QuickUploadPrepTests {
    @Test func rejectsFilesThatAreNeitherImagesNorDocuments() async {
        #expect(await prepare(Data(), mimeType: "image/png") == nil)
        #expect(await prepare(Data("definitely not an image".utf8), mimeType: "image/png") == nil)
        #expect(await prepare(Data("{}".utf8), mimeType: "application/json") == nil)
    }

    @Test func documentsKeepTheirBytesAndMimeTypeWithoutAThumbnail() async throws {
        let pdf = Data("%PDF-1.7 receipt".utf8)
        let csv = Data("date,amount\n2026-10-01,-12.50\n".utf8)

        let preparedPdf = try #require(await prepare(pdf, mimeType: "application/pdf"))
        let preparedCsv = try #require(await prepare(csv, mimeType: "text/csv"))

        #expect(preparedPdf.data == pdf)
        #expect(preparedPdf.mimeType == "application/pdf")
        #expect(preparedPdf.thumbnail.isEmpty)
        #expect(preparedCsv.data == csv)
        #expect(preparedCsv.mimeType == "text/csv")
        #expect(preparedCsv.thumbnail.isEmpty)
    }

    @Test func imagesKeepTheirBytesWithTheMimeTypeOfTheirContent() async throws {
        let png = try pngData(width: 400, height: 300)

        let prepared = try #require(await prepare(png, mimeType: "application/octet-stream"))

        #expect(prepared.data == png)
        #expect(prepared.mimeType == "image/png")
    }

    @Test func heicIsReencodedAsJpeg() async throws {
        let heic = try heicData(from: pngData(width: 400, height: 300))

        let prepared = try #require(await prepare(heic, mimeType: "image/heic"))

        #expect(isJpeg(prepared.data))
        #expect(prepared.mimeType == "image/jpeg")
    }
}
