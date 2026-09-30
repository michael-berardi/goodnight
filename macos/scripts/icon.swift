// Renders the app icon (1024 px PNG) with CoreGraphics. Usage: swift scripts/icon.swift out.png
import AppKit

let size = 1024.0
let out = CommandLine.arguments.dropFirst().first ?? "icon.png"
let cs = CGColorSpace(name: CGColorSpace.displayP3)!
let ctx = CGContext(data: nil, width: Int(size), height: Int(size), bitsPerComponent: 8, bytesPerRow: 0,
                    space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
func rgb(_ r: Double, _ g: Double, _ b: Double, _ a: Double = 1) -> CGColor { CGColor(colorSpace: cs, components: [r, g, b, a])! }

// macOS icon grid: 824 px body, continuous corners, soft drop shadow.
let body = CGRect(x: 100, y: 100, width: 824, height: 824)
let shape = NSBezierPath(roundedRect: body, xRadius: 185, yRadius: 185).cgPath
ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: rgb(0, 0, 0, 0.35))
ctx.addPath(shape); ctx.setFillColor(rgb(0.05, 0.05, 0.14)); ctx.fillPath()
ctx.restoreGState()

ctx.saveGState()
ctx.addPath(shape); ctx.clip()

// Night sky, deepest at the top.
let sky = CGGradient(colorsSpace: cs, colors: [rgb(0.035, 0.04, 0.12), rgb(0.09, 0.07, 0.22), rgb(0.24, 0.12, 0.30)] as CFArray,
                     locations: [0, 0.6, 1])!
ctx.drawLinearGradient(sky, start: CGPoint(x: 0, y: 924), end: CGPoint(x: 0, y: 100), options: [])

// The sun just below the horizon.
let glow = CGGradient(colorsSpace: cs, colors: [rgb(1, 0.52, 0.26, 0.85), rgb(0.95, 0.36, 0.30, 0.35), rgb(0.6, 0.2, 0.35, 0)] as CFArray,
                      locations: [0, 0.45, 1])!
ctx.drawRadialGradient(glow, startCenter: CGPoint(x: 512, y: 20), startRadius: 0, endCenter: CGPoint(x: 512, y: 20), endRadius: 560, options: [])

// A few stars.
var seed: UInt64 = 5
for _ in 0..<16 {
    seed = seed &* 6364136223846793005 &+ 1442695040888963407
    let x = 150 + Double(seed >> 40 & 0xFFFF) / 65535 * 724
    let y = 560 + Double(seed >> 20 & 0xFFFF) / 65535 * 330
    let r = 4 + Double(seed >> 8 & 0xFF) / 255 * 6
    ctx.setFillColor(rgb(1, 0.96, 0.9, 0.35 + Double(seed & 0xFF) / 255 * 0.45))
    ctx.fillEllipse(in: CGRect(x: x - r / 2, y: y - r / 2, width: r, height: r))
}

// Crescent: outer disc minus an offset disc, lit from the warm horizon below.
let c = CGPoint(x: 505, y: 520), r = 255.0
let outer = CGPath(ellipseIn: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r), transform: nil)
let bite = CGPath(ellipseIn: CGRect(x: c.x - r + 175, y: c.y - r + 130, width: 2 * r * 0.9, height: 2 * r * 0.9), transform: nil)
func crescent() {
    ctx.addPath(outer); ctx.clip()
    ctx.addRect(body.insetBy(dx: -100, dy: -100)); ctx.addPath(bite); ctx.clip(using: .evenOdd)
}
// Glow: draw the crescent shape blurred behind itself.
ctx.saveGState()
ctx.setShadow(offset: .zero, blur: 110, color: rgb(1, 0.7, 0.45, 0.75))
ctx.beginTransparencyLayer(auxiliaryInfo: nil)
crescent()
ctx.setFillColor(rgb(1, 0.85, 0.7))
ctx.fill(body)
ctx.endTransparencyLayer()
ctx.restoreGState()
// Face: cream at the tips, amber where it faces the horizon.
ctx.saveGState()
crescent()
let face = CGGradient(colorsSpace: cs, colors: [rgb(1, 0.97, 0.9), rgb(1, 0.86, 0.66), rgb(1, 0.6, 0.34)] as CFArray, locations: [0, 0.55, 1])!
ctx.drawLinearGradient(face, start: CGPoint(x: 640, y: 780), end: CGPoint(x: 300, y: 280), options: [])
ctx.restoreGState()

// Soft top highlight.
let gloss = CGGradient(colorsSpace: cs, colors: [rgb(1, 1, 1, 0.10), rgb(1, 1, 1, 0)] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(gloss, start: CGPoint(x: 0, y: 924), end: CGPoint(x: 0, y: 660), options: [])
ctx.restoreGState()

let rep = NSBitmapImageRep(cgImage: ctx.makeImage()!)
try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out))
