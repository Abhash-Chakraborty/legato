import CoreText
import CryptoKit
import LegatoCore
import UIKit
import XCTest

/// Hears the core and keeps what it said.
private final class Listener: CoreListener, @unchecked Sendable {
    private let lock = NSLock()
    private var said: [String] = []
    private var ended: [String] = []
    private let vault: Vault?

    init(vault: Vault? = nil) {
        self.vault = vault
    }

    var lines: [String] {
        lock.lock()
        defer { lock.unlock() }
        return said
    }

    var endings: [String] {
        lock.lock()
        defer { lock.unlock() }
        return ended
    }

    func onLine(line: String) {
        lock.lock()
        said.append(line)
        lock.unlock()
    }

    func onUnpaired(why: String) {
        lock.lock()
        ended.append(why)
        lock.unlock()
    }

    func openBluetooth(address: String, token: String) -> UInt16 {
        0
    }

    func onClip(text: String, fromApp: String?, ageMs: UInt64) {}

    func onAiKeys(keys: [ProviderKey]) {}

    func seal(plain: Data) -> Data? {
        vault?.seal(plain)
    }

    func unseal(sealed: Data) -> Data? {
        vault?.open(sealed)
    }
}

/// The shared core on iPhone: the same library Android runs, opened in a
/// temporary folder and driven through its Swift bindings.
final class CoreTests: XCTestCase {
    private var dir: URL!
    private var listener: Listener!
    private var core: Core!

    override func setUpWithError() throws {
        dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("legato-core-\(UUID().uuidString)", isDirectory: true)
        listener = Listener()
        core = try Core(dir: dir.path, name: "Test iPhone", listener: listener)
    }

    override func tearDown() {
        core?.disconnect()
        core = nil
        try? FileManager.default.removeItem(at: dir)
    }

    private struct TimedOut: Error, CustomStringConvertible {
        let description: String
    }

    /// Looks every tenth of a second until `done`, for up to `seconds`.
    private func eventually(_ what: String, seconds: TimeInterval = 20, _ done: () throws -> Bool) throws {
        let end = Date().addingTimeInterval(seconds)
        while try !done() {
            if Date() > end {
                throw TimedOut(description: "Timed out waiting for \(what). The core said: \(listener.lines.suffix(8))")
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
    }

    func testTheCoreAnswersAndStartsUnpaired() {
        XCTAssertFalse(coreVersion().isEmpty)
        let state = core.state()
        XCTAssertFalse(state.paired)
        XCTAssertFalse(state.online)
        XCTAssertNil(state.pcName)
        XCTAssertNil(state.lastSync)
        XCTAssertTrue(FileManager.default.fileExists(atPath: dir.appendingPathComponent("phone.db").path))
    }

    func testAPairingLinkIsReadBeforeAnythingChanges() throws {
        let link = "legato://pair?v=1&id=6f1c&name=Desk%20PC&fp=q-_x"
            + "&addr=192.168.1.20:47613,100.64.12.40:47613,bt:AA:BB:CC:DD:EE:FF&secret=s3cr3t"
        let preview = try core.inspect(link: link)
        XCTAssertEqual(preview.pcName, "Desk PC")
        XCTAssertEqual(preview.addresses, ["192.168.1.20:47613", "100.64.12.40:47613", "Bluetooth"])
        XCTAssertNil(preview.replaces)
        XCTAssertFalse(core.state().paired, "inspect must not pair")

        // A link to a public address is someone else's "PC".
        let elsewhere = link.replacingOccurrences(of: "192.168.1.20", with: "203.0.113.5")
        XCTAssertThrowsError(try core.inspect(link: elsewhere)) { error in
            XCTAssertTrue(coreWords(error).contains("outside your network"), coreWords(error))
        }
        XCTAssertThrowsError(try core.inspect(link: "https://example.com")) { error in
            XCTAssertEqual(coreWords(error), "not a Legato pairing link")
        }
        XCTAssertThrowsError(try core.connect(), "connect needs a paired PC")
    }

    func testDictionaryWordsWithWhatTheyAreHeardAs() throws {
        try core.addWordHeard(text: "Okonkwo", heard: "Oh Conk Woe")
        try core.addWordHeard(text: "Okonkwo", heard: "o conquer")
        try core.addWordHeard(text: "Tallinn", heard: "")
        let rows = try core.wordRows()
        XCTAssertEqual(rows.map(\.text), ["Okonkwo", "Tallinn"])
        XCTAssertEqual(Set(rows[0].heard), ["oh conk woe", "o conquer"])
        XCTAssertEqual(rows[1].heard, [])
        XCTAssertEqual(try core.words(), ["Okonkwo", "Tallinn"])

        try core.removeWord(text: "okonkwo")
        XCTAssertEqual(try core.wordRows().map(\.text), ["Tallinn"])
    }

    func testSnippetsAreAddedEditedAndRemoved() throws {
        try core.saveSnippet(id: nil, trigger: "My Address", text: "14 Linden Road")
        var rows = try core.snippetRows()
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].trigger, "my address")
        XCTAssertEqual(rows[0].text, "14 Linden Road")

        try core.saveSnippet(id: rows[0].id, trigger: "my address", text: "14 Linden Road, Bristol")
        rows = try core.snippetRows()
        XCTAssertEqual(rows.map(\.text), ["14 Linden Road, Bristol"])

        try core.removeSnippet(id: rows[0].id)
        XCTAssertTrue(try core.snippetRows().isEmpty)
    }

    func testScratchpadNotesAreSavedChangedAndDeleted() throws {
        let id = try core.saveScratch(id: nil, title: "Packing list", text: "Passport")
        var notes = try core.scratchNotes()
        XCTAssertEqual(notes.map(\.title), ["Packing list"])
        XCTAssertEqual(notes[0].id, id)

        let same = try core.saveScratch(id: id, title: "Packing list", text: "Passport\nCharger")
        XCTAssertEqual(same, id)
        notes = try core.scratchNotes()
        XCTAssertEqual(notes.count, 1)
        XCTAssertEqual(notes[0].text, "Passport\nCharger")

        try core.deleteScratch(id: id)
        XCTAssertTrue(try core.scratchNotes().isEmpty)
    }

    func testDictationsAreListedDeletedAndPutBack() throws {
        try core.dictate(text: "See you at noon.", app: "Messages")
        try core.keepCancelled(text: "Never mind.", app: "")
        var history = try core.history(limit: 10)
        XCTAssertEqual(history.count, 2)
        let noon = try XCTUnwrap(history.first { $0.text == "See you at noon." })
        XCTAssertEqual(noon.app, "Messages")
        XCTAssertNil(noon.device, "a dictation made here has no other device's name")
        XCTAssertFalse(noon.cancelled)
        let kept = try XCTUnwrap(history.first { $0.text == "Never mind." })
        XCTAssertTrue(kept.cancelled)
        XCTAssertEqual(kept.app, "Phone")

        try core.deleteHistory(id: noon.id)
        XCTAssertEqual(try core.history(limit: 10).map(\.text), ["Never mind."])

        // Undo puts it back with the time it had.
        try core.restoreHistory(text: noon.text, app: noon.app, at: noon.at - 3600, cancelled: false)
        history = try core.history(limit: 10)
        XCTAssertEqual(history.map(\.text), ["Never mind.", "See you at noon."])
        XCTAssertEqual(history[1].at, noon.at - 3600)
    }

    func testAMemoBecomesANoteWithItsTranscript() throws {
        let id = try core.addMemo(
            title: "Shopping",
            startedAt: 1_800_000_000,
            durationMs: 41_000,
            lines: [
                MemoLine(atMs: 0, endMs: 20_000, text: "Tomatoes and basil."),
                MemoLine(atMs: 20_000, endMs: 41_000, text: "  "),
                MemoLine(atMs: 20_000, endMs: 41_000, text: "Bread from the corner."),
            ]
        )
        let notes = try core.notes()
        XCTAssertEqual(notes.map(\.title), ["Shopping"])
        XCTAssertEqual(notes[0].durationMs, 41_000)
        XCTAssertNil(notes[0].device)
        XCTAssertEqual(notes[0].openActions, 0)

        let detail = try core.note(id: id)
        XCTAssertEqual(detail.transcript, ["you: Tomatoes and basil.", "you: Bread from the corner."])
        XCTAssertTrue(detail.actions.isEmpty)

        XCTAssertTrue(try core.deleteNote(id: id), "a note made here can be deleted here")
        XCTAssertTrue(try core.notes().isEmpty)
        XCTAssertThrowsError(try core.note(id: id))
    }

    func testThePrivateKeyIsSealedAtRest() throws {
        let sealedDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("legato-sealed-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: sealedDir) }
        let vault = Vault(fixedKey: SymmetricKey(size: .bits256))

        let first = try Core(dir: sealedDir.path, name: "Test iPhone", listener: Listener(vault: vault))
        _ = first.state()
        let sealed = sealedDir.appendingPathComponent("identity.sealed")
        XCTAssertTrue(FileManager.default.fileExists(atPath: sealed.path))
        XCTAssertFalse(FileManager.default.fileExists(atPath: sealedDir.appendingPathComponent("identity.json").path))
        let bytes = try Data(contentsOf: sealed)
        XCTAssertNil(bytes.range(of: Data("device_id".utf8)), "the key file must not be readable")
        XCTAssertNotNil(vault.open(bytes))

        // Opened again with the same key, the identity is the one from before.
        let again = Listener(vault: vault)
        _ = try Core(dir: sealedDir.path, name: "Test iPhone", listener: again)
        XCTAssertEqual(try Data(contentsOf: sealed), bytes)
        XCTAssertTrue(again.lines.isEmpty, "\(again.lines)")

        // A key that can't open it (the phone was reset) makes a new identity, and says so.
        let other = Listener(vault: Vault(fixedKey: SymmetricKey(size: .bits256)))
        _ = try Core(dir: sealedDir.path, name: "Test iPhone", listener: other)
        XCTAssertNotEqual(try Data(contentsOf: sealed), bytes)
        XCTAssertTrue(other.lines.contains { $0.contains("pair with your PC again") }, "\(other.lines)")
    }

    /// Pairs with a stand-in PC on this Mac (phone/stand-in-pc), when the
    /// run started one: the simulator shares the Mac's network.
    func testItPairsAndSyncsBothWaysWithAStandInPC() throws {
        guard let link = ProcessInfo.processInfo.environment["LEGATO_PAIR_LINK_CORE"], !link.isEmpty else {
            throw XCTSkip("No stand-in PC is running (LEGATO_PAIR_LINK_CORE isn't set).")
        }
        let preview = try core.inspect(link: link)
        XCTAssertEqual(preview.pcName, "Desk PC")

        try core.pair(link: link)
        XCTAssertTrue(core.state().paired)
        try eventually("the link to come up") { core.state().online }
        XCTAssertEqual(core.state().pcName, "Desk PC")
        XCTAssertEqual(core.state().via, "lan")

        // PC to phone: its Dictionary, dictations, Scratchpad and a meeting.
        try eventually("the PC's words") { try core.wordRows().contains { $0.text == "Okonkwo" } }
        try eventually("the PC's dictations") { try core.history(limit: 50).count >= 3 }
        try eventually("the PC's Scratchpad note") { try core.scratchNotes().contains { $0.title == "Reading list" } }
        try eventually("the PC's meeting") { try core.notes().contains { $0.title == "Garden club planning" } }

        let okonkwo = try XCTUnwrap(core.wordRows().first { $0.text == "Okonkwo" })
        XCTAssertEqual(okonkwo.heard, ["oh conk woe"])
        let fromPc = try core.history(limit: 50)
        XCTAssertTrue(fromPc.allSatisfy { $0.device == "Desk PC" }, "\(fromPc)")
        let meeting = try XCTUnwrap(core.notes().first { $0.title == "Garden club planning" })
        XCTAssertEqual(meeting.device, "Desk PC")
        XCTAssertEqual(meeting.openActions, 3)
        XCTAssertFalse(meeting.summary.isEmpty)
        let detail = try core.note(id: meeting.id)
        XCTAssertEqual(detail.actions.count, 3)
        XCTAssertEqual(detail.transcript.count, 4)
        XCTAssertFalse(try core.deleteNote(id: meeting.id), "the PC's note is deleted on the PC")
        try eventually("the first sync to be timed") { core.state().lastSync != nil }

        // Phone to PC: a word, a ticked action item and a dictation. The
        // stand-in prints what it holds; the workflow reads its log.
        try core.addWordHeard(text: "Zephyrine", heard: "zeff a reen")
        try core.setActionDone(id: detail.actions[0].id, done: true)
        try core.dictate(text: "Sent from the simulator.", app: "Tests")
        XCTAssertEqual(try core.note(id: meeting.id).note.openActions, 2)
        XCTAssertTrue(try core.syncNow(), "sync now reaches a connected PC")
        try eventually("the PC to take the changes") { listener.lines.contains { $0.hasPrefix("Sent ") } }
        Thread.sleep(forTimeInterval: 3)

        XCTAssertTrue(listener.lines.contains { $0.hasPrefix("Paired with Desk PC") }, "\(listener.lines)")
        XCTAssertTrue(listener.endings.isEmpty, "\(listener.endings)")
        core.disconnect()
        try eventually("the link to close") { !core.state().online }
        XCTAssertTrue(core.state().paired, "disconnecting keeps the pairing")
    }
}

/// The look and the words the screens share, checked without the app.
final class LookTests: XCTestCase {
    func testFigtreeComesInEveryWeightTheScreensUse() {
        // The app lists the font in its Info.plist; this bundle carries the
        // same file and registers it itself.
        if UIFont.fontNames(forFamilyName: "Figtree").isEmpty,
           let file = Bundle(for: LookTests.self).url(forResource: "Figtree", withExtension: "ttf") {
            CTFontManagerRegisterFontsForURL(file as CFURL, .process, nil)
        }
        XCTAssertTrue(Theme.registerFont(), "Figtree.ttf should be found and registered")
        let regular = Theme.uiFont(size: 20, weight: 400)
        let semibold = Theme.uiFont(size: 20, weight: 600)
        let bold = Theme.uiFont(size: 20, weight: 700)
        XCTAssertEqual(regular.familyName, "Figtree")
        XCTAssertEqual(bold.familyName, "Figtree")
        let sample = "Legato writes what you say" as NSString
        func width(_ font: UIFont) -> CGFloat {
            sample.size(withAttributes: [.font: font]).width
        }
        // A heavier weight on the variable font's axis sets wider.
        XCTAssertGreaterThan(width(semibold), width(regular) + 0.5)
        XCTAssertGreaterThan(width(bold), width(semibold) + 0.5)
        print("Figtree: \(regular.fontName) \(width(regular)), \(semibold.fontName) \(width(semibold)), \(bold.fontName) \(width(bold))")
    }

    func testTimesReadAsOnAndroid() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        XCTAssertEqual(ago(1_800_000_000 - 20, now: now), "just now")
        XCTAssertEqual(ago(1_800_000_000 - 5 * 60, now: now), "5 min ago")
        XCTAssertEqual(ago(1_800_000_000 - 3 * 3600, now: now), "3 h ago")
        XCTAssertEqual(ago(1_800_000_000 - 2 * 86400, now: now), "2 d ago")
        XCTAssertEqual(minutes(20_000), "under a minute")
        XCTAssertEqual(minutes(24 * 60_000), "24 min")
        XCTAssertEqual(minutes(65 * 60_000), "1 h 5 min")
        XCTAssertEqual(dayLabel(1_800_000_000, now: now), "Today")
        XCTAssertEqual(dayLabel(1_800_000_000 - 86400, now: now), "Yesterday")
        XCTAssertNil(openActions(0))
        XCTAssertEqual(openActions(1), "1 open action")
        XCTAssertEqual(openActions(3), "3 open actions")
    }

    func testThePcStatusHasItsOwnWordsForEachState() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        var state = PhoneState(paired: false, pcName: nil, online: false, via: nil, pcTranscribes: false, lastSync: nil)
        XCTAssertEqual(pcStatus(state, device: "iPhone", now: now), PcStatus(mood: .off, text: "Not paired"))

        state = PhoneState(
            paired: true, pcName: "Desk PC", online: true, via: "lan", pcTranscribes: false, lastSync: 1_800_000_000 - 120
        )
        XCTAssertEqual(
            pcStatus(state, device: "iPhone", now: now),
            PcStatus(mood: .connected, text: "Desk PC · synced 2 min ago")
        )
        XCTAssertEqual(
            pcStatus(state, device: "iPhone", long: true, now: now),
            PcStatus(mood: .connected, text: "Connected on your network · synced 2 min ago")
        )
        state.via = "netbird"
        state.lastSync = nil
        XCTAssertEqual(
            pcStatus(state, device: "iPhone", long: true, now: now).text, "Connected through your VPN"
        )
        XCTAssertEqual(pcStatus(state, device: "iPhone", now: now).text, "Desk PC · connected")

        state.online = false
        XCTAssertEqual(
            pcStatus(state, device: "iPhone", now: now),
            PcStatus(mood: .trying, text: "Desk PC not reachable · changes wait here")
        )
        XCTAssertEqual(
            pcStatus(state, device: "iPhone", long: true, now: now).text,
            "Can't reach it right now · changes wait on this iPhone"
        )
    }

    func testAScratchpadNoteGetsAHeadingAndAPreview() {
        XCTAssertEqual(scratchHeading(title: "Packing list", text: "Passport\nCharger"), "Packing list")
        XCTAssertEqual(scratchPreview(title: "Packing list", text: "Passport\nCharger"), "Passport Charger")
        XCTAssertEqual(scratchHeading(title: "", text: "\n  Ideas for the garden\nRaised beds"), "Ideas for the garden")
        XCTAssertEqual(scratchPreview(title: "", text: "Ideas for the garden\nRaised beds"), "Raised beds")
        XCTAssertEqual(scratchHeading(title: " ", text: ""), "Untitled")
        // A title that is only the start of a word isn't the start of the text.
        XCTAssertEqual(scratchPreview(title: "Ok", text: "Okay, see you then"), "Okay, see you then")
        XCTAssertEqual(scratchPreview(title: "Okay", text: "Okay, see you then"), "see you then")

        let long = "Ideas for the garden: raised beds along the south fence, a water butt by the shed, and herbs near the kitchen door."
        let heading = scratchHeading(title: "", text: long)
        XCTAssertTrue(heading.hasSuffix("…"))
        XCTAssertLessThanOrEqual(heading.count, 81)
        XCTAssertFalse(heading.dropLast().hasSuffix(" "))
        XCTAssertTrue(long.hasPrefix(heading.dropLast()))
        XCTAssertTrue(long.hasSuffix(scratchPreview(title: "", text: long)))
    }

    func testFailuresAreSaidInPlainWords() {
        XCTAssertEqual(plainWords("", pc: nil), "That didn't work. Try again.")
        XCTAssertTrue(plainWords("connection refused (os error 61)", pc: "Desk PC").hasPrefix("Couldn't reach Desk PC."))
        XCTAssertEqual(plainWords("database is locked", pc: nil), "Legato was busy for a moment. Try again.")
        XCTAssertEqual(plainWords("That note is gone", pc: nil), "That note is gone")
        XCTAssertEqual(plainWords("std::io::Error(Thing)", pc: nil), "That didn't work. Try again.")
    }

    func testTheVaultSealsAndOpensOnlyWithItsKey() throws {
        let vault = Vault(fixedKey: SymmetricKey(size: .bits256))
        let plain = Data("a private key".utf8)
        let sealed = try XCTUnwrap(vault.seal(plain))
        XCTAssertNotEqual(sealed, plain)
        XCTAssertNil(sealed.range(of: plain))
        XCTAssertEqual(vault.open(sealed), plain)
        XCTAssertNil(Vault(fixedKey: SymmetricKey(size: .bits256)).open(sealed))
        XCTAssertNil(vault.open(Data("not sealed".utf8)))
    }
}
