#if DEBUG
import Foundation
import LegatoCore
import SwiftUI

/// What a UI test asks for when it starts the app. Debug builds only.
///
///   -freshData           keep this run's files in a new temporary folder
///   -seedDemo            the same, filled with made-up content through the core
///   -appearance dark     dark or light, whatever the simulator is set to
struct Launch {
    var dataDir: URL?
    var seed = false
    var look: ColorScheme?

    init(arguments: [String]) {
        seed = arguments.contains("-seedDemo")
        if seed || arguments.contains("-freshData") {
            dataDir = FileManager.default.temporaryDirectory
                .appendingPathComponent("legato-\(UUID().uuidString)", isDirectory: true)
        }
        if let at = arguments.firstIndex(of: "-appearance"), arguments.indices.contains(at + 1) {
            look = arguments[at + 1] == "dark" ? .dark : .light
        }
    }
}

/// Made-up words, dictations and notes for screenshots, written through the
/// core exactly as the screens write them.
enum Demo {
    static func seed(_ store: Store) {
        guard let core = store.core else { return }
        let now = Int64(Date().timeIntervalSince1970)
        do {
            try core.addWordHeard(text: "Okonkwo", heard: "oh conk woe")
            try core.addWordHeard(text: "Solveig", heard: "sol vague")
            try core.addWordHeard(text: "Kubernetes", heard: "cooper netties")
            try core.addWordHeard(text: "Tallinn", heard: "")
            try core.addWordHeard(text: "Figma", heard: "")

            try core.saveSnippet(id: nil, trigger: "my address", text: "14 Linden Road, Bristol BS6 5QT")
            try core.saveSnippet(id: nil, trigger: "sign off", text: "Thanks, and talk soon.\nMaya")
            try core.saveSnippet(
                id: nil, trigger: "booking link", text: "You can pick a time that suits you at example.com/meet/maya"
            )

            try core.restoreHistory(
                text: "Can we move lunch to one? The dentist is running late.",
                app: "Messages", at: now - 12 * 60, cancelled: false
            )
            try core.restoreHistory(
                text: "Thanks for the notes. I'll send the revised quote by Thursday and copy Tomas in.",
                app: "Mail", at: now - 2 * 3600, cancelled: false
            )
            try core.restoreHistory(
                text: "Remind me to water the plants before the weekend.",
                app: "Reminders", at: now - 26 * 3600, cancelled: false
            )
            try core.restoreHistory(
                text: "Never mind, I found the address.",
                app: "Messages", at: now - 27 * 3600, cancelled: true
            )
            try core.restoreHistory(
                text: "The second paragraph should open with the survey result, not the method.",
                app: "Pages", at: now - 4 * 86400, cancelled: false
            )

            _ = try core.saveScratch(
                id: nil, title: "Packing list", text: "Passport\nCharger\nRain jacket\nA book for the train"
            )
            _ = try core.saveScratch(
                id: nil, title: "",
                text: "Ideas for the garden: raised beds along the south fence, a water butt by the shed, and herbs near the kitchen door."
            )
            _ = try core.saveScratch(
                id: nil, title: "Gift ideas", text: "A cast-iron pan for Tomas\nConcert tickets for Ines"
            )

            _ = try core.addMemo(
                title: "Thoughts after the call",
                startedAt: now - 3600,
                durationMs: 95_000,
                lines: [
                    MemoLine(
                        atMs: 0, endMs: 30_000,
                        text: "The quote is fine as long as delivery stays in March. Ask whether the second batch can ship with the first."
                    ),
                    MemoLine(
                        atMs: 30_000, endMs: 62_000,
                        text: "Ines wants the colour samples before she decides, so send those tomorrow morning."
                    ),
                    MemoLine(
                        atMs: 62_000, endMs: 95_000,
                        text: "Check the invoice address, it still shows the old office."
                    ),
                ]
            )
            _ = try core.addMemo(
                title: "Shopping for Saturday",
                startedAt: now - 28 * 3600,
                durationMs: 41_000,
                lines: [
                    MemoLine(
                        atMs: 0, endMs: 41_000,
                        text: "Tomatoes, basil, two lemons, olive oil, and bread from the bakery on the corner."
                    ),
                ]
            )
        } catch {
            store.notice(coreWords(error))
        }
        store.refresh()
    }
}
#endif
