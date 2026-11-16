import SwiftUI
import UIKit

@main
struct LegatoApp: App {
    @StateObject private var store: Store
    /// Light or dark when a test asks for one; else the system's.
    private let look: ColorScheme?

    init() {
        Theme.registerFont()
        let device = UIDevice.current.userInterfaceIdiom == .pad ? "iPad" : "iPhone"
        let name = UIDevice.current.name
        #if DEBUG
        let launch = Launch(arguments: ProcessInfo.processInfo.arguments)
        let store = Store(device: device, name: name, dataDir: launch.dataDir)
        if launch.seed { Demo.seed(store) }
        look = launch.look
        #else
        let store = Store(device: device, name: name, dataDir: nil)
        look = nil
        #endif
        _store = StateObject(wrappedValue: store)
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(store)
                .preferredColorScheme(look)
        }
    }
}
