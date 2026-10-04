// Stores user-picked theme photos as downscaled JPEGs in the App Group container.

import ImageIO
import UIKit

/// Theme photos live in `<appGroupContainer>/theme_images/<uuid>.jpg` so the host app
/// (writer, via the photo picker) and the keyboard extension (reader) share them. A picked
/// photo is downsampled while decoding (ImageIO thumbnail, so a 48 MP original never becomes
/// a full bitmap) to `maxLongEdge` and re-encoded on save, which bounds the extension's
/// decode cost regardless of the original's size; the directory is excluded from OS backup
/// like the other user-data files. Save is safe off the main actor.
final class ThemeImageStore {
    static let directoryName = "theme_images"
    /// Longest edge after downsampling — wider than any phone keyboard at 3× yet ~5 MB decoded.
    static let maxLongEdge: CGFloat = 1280
    static let jpegQuality: CGFloat = 0.85

    private let directoryURL: URL?

    /// - Parameter containerURL: App Group container (nil if provisioning failed → the
    ///   store degrades to "no photos", never crashes).
    init(containerURL: URL?) {
        directoryURL = containerURL?.appendingPathComponent(Self.directoryName, isDirectory: true)
    }

    func url(for file: String) -> URL? {
        directoryURL?.appendingPathComponent(file)
    }

    /// Downsamples, encodes and writes the picked photo's encoded `data`; returns the new
    /// file name, or nil when the data is not an image, the container is unavailable or
    /// the write fails.
    func save(_ data: Data) -> String? {
        guard let directoryURL,
              let image = Self.downsampled(data, maxLongEdge: Self.maxLongEdge),
              let jpeg = image.jpegData(compressionQuality: Self.jpegQuality) else { return nil }
        let file = UUID().uuidString + ".jpg"
        do {
            try FileManager.default.createDirectory(at: directoryURL, withIntermediateDirectories: true)
            directoryURL.excludeFromBackup()
            try jpeg.write(to: directoryURL.appendingPathComponent(file), options: .atomic)
            return file
        } catch {
            return nil
        }
    }

    /// Removes every stored photo whose name is not in `referenced` (the files the saved
    /// user themes still point at).
    func sweep(keeping referenced: Set<String>) {
        guard let directoryURL,
              let files = try? FileManager.default.contentsOfDirectory(atPath: directoryURL.path) else { return }
        for file in files where !referenced.contains(file) {
            try? FileManager.default.removeItem(at: directoryURL.appendingPathComponent(file))
        }
    }

    /// Decodes `data` straight into a bitmap whose longer edge is at most `maxLongEdge`
    /// (never upscaled), with the EXIF orientation baked into the pixels so the extension
    /// can draw it as-is. nil when `data` is not a decodable image.
    static func downsampled(_ data: Data, maxLongEdge: CGFloat) -> UIImage? {
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxLongEdge,
        ]
        guard let source = CGImageSourceCreateWithData(data as CFData, nil),
              let cgImage = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else { return nil }
        return UIImage(cgImage: cgImage)
    }
}
