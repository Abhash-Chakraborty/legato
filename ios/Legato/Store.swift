import Combine
import Foundation
import LegatoCore

/// A message at the bottom of the screen, with an action when there is one
/// to offer (Undo).
struct Toast: Identifiable {
    let id = UUID()
    let text: String
    var actionLabel: String?
    var action: (() -> Void)?
    /// Runs when the toast goes without its action having been tapped.
    var expired: (() -> Void)?
}

/// What the core says while it works, passed on to the store. The core
/// calls these from its own threads.
final class Bridge: CoreListener, @unchecked Sendable {
    private let vault: Vault
    private let lock = NSLock()
    private var early: [String] = []
    private var said: ((String) -> Void)?
    private var unpaired: ((String) -> Void)?

    init(vault: Vault) {
        self.vault = vault
    }

    /// Starts passing lines on, the ones from before this was called first.
    func attach(said: @escaping (String) -> Void, unpaired: @escaping (String) -> Void) {
        lock.lock()
        let waiting = early
        early = []
        self.said = said
        self.unpaired = unpaired
        lock.unlock()
        waiting.forEach(said)
    }

    func onLine(line: String) {
        lock.lock()
        let said = self.said
        if said == nil { early.append(line) }
        lock.unlock()
        said?(line)
    }

    func onUnpaired(why: String) {
        lock.lock()
        let unpaired = self.unpaired
        lock.unlock()
        unpaired?(why)
    }

    /// iOS gives apps no Bluetooth Classic, so there is never a port: the
    /// core then leaves the PC's Bluetooth address alone.
    func openBluetooth(address: String, token: String) -> UInt16 {
        0
    }

    // The shared clipboard and transforms aren't on iPhone yet: clips and
    // AI keys the PC sends are dropped here and kept nowhere.
    func onClip(text: String, fromApp: String?, ageMs: UInt64) {}

    func onAiKeys(keys: [ProviderKey]) {}

    func seal(plain: Data) -> Data? {
        vault.seal(plain)
    }

    func unseal(sealed: Data) -> Data? {
        vault.open(sealed)
    }
}

/// The app's one door to the core: it opens the core, keeps what the
/// screens show (the link's state and each list) up to date, and runs
/// edits off the main thread. Everything published changes on the main
/// thread only.
final class Store: ObservableObject {
    @Published private(set) var state: PhoneState
    @Published private(set) var history: [HistoryEntry] = []
    @Published private(set) var words: [WordRow] = []
    @Published private(set) var snippets: [SnippetRow] = []
    @Published private(set) var scratch: [ScratchNote] = []
    @Published private(set) var notes: [NoteRow] = []
    /// The log: what the core did, oldest first.
    @Published private(set) var lines: [String] = []
    /// Goes up after every edit and every line from the core, for screens
    /// that read one item themselves.
    @Published private(set) var version = 0
    @Published private(set) var toast: Toast?
    /// Why the PC ended or refused the pairing, until this phone pairs again.
    @Published private(set) var ended: String?

    /// "iPhone" or "iPad", for the words on screen.
    let device: String
    /// Nil when the core couldn't open its files; `failure` says why.
    let core: Core?
    let failure: String?
    /// Whether this iPhone's private key is sealed with the Keychain's key.
    let keySealed: Bool

    private let dir: URL
    private let bridge: Bridge
    /// One thing at a time, in order: the core's calls block.
    private let work = DispatchQueue(label: "tech.abhashchakraborty.legato.core")
    private var wasOnline = false

    private static let logLines = 400
    private static let unpaired = PhoneState(
        paired: false, pcName: nil, online: false, via: nil, pcTranscribes: false, lastSync: nil
    )

    /// `name` is what the PC shows for this phone. `dataDir` is for tests;
    /// nil keeps the files in the app's own folder.
    init(device: String, name: String, dataDir: URL?) {
        self.device = device
        let dir = dataDir ?? Store.home()
        self.dir = dir
        Store.prepare(dir)
        let bridge = Bridge(vault: Vault())
        self.bridge = bridge
        var core: Core?
        var failure: String?
        do {
            core = try Core(dir: dir.path, name: name, listener: bridge)
        } catch {
            failure = coreWords(error)
        }
        self.core = core
        self.failure = failure
        self.state = core?.state() ?? Store.unpaired
        self.keySealed = FileManager.default.fileExists(atPath: dir.appendingPathComponent("identity.sealed").path)
        self.lines = Store.readLog(dir)
        bridge.attach(
            said: { [weak self] line in self?.heard(line) },
            unpaired: { [weak self] why in self?.pairingEnded(why) }
        )
        if let core {
            apply(Store.read(core), bump: false)
        }
    }

    // MARK: Reading

    private struct Snapshot {
        var state: PhoneState
        var history: [HistoryEntry]?
        var words: [WordRow]?
        var snippets: [SnippetRow]?
        var scratch: [ScratchNote]?
        var notes: [NoteRow]?
    }

    private static func read(_ core: Core) -> Snapshot {
        // A dictation that came out empty on the PC has nothing to show or copy.
        let history = (try? core.history(limit: 200))?.filter {
            !$0.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        return Snapshot(
            state: core.state(),
            history: history,
            words: try? core.wordRows(),
            snippets: try? core.snippetRows(),
            scratch: try? core.scratchNotes(),
            notes: try? core.notes()
        )
    }

    /// On the main thread. A list that couldn't be read keeps what it had.
    private func apply(_ snapshot: Snapshot, bump: Bool) {
        if state != snapshot.state { state = snapshot.state }
        if state.paired, ended != nil { ended = nil }
        if let history = snapshot.history, history != self.history { self.history = history }
        if let words = snapshot.words, words != self.words { self.words = words }
        if let snippets = snapshot.snippets, snippets != self.snippets { self.snippets = snippets }
        if let scratch = snapshot.scratch, scratch != self.scratch { self.scratch = scratch }
        if let notes = snapshot.notes, notes != self.notes { self.notes = notes }
        if bump { version += 1 }
    }

    /// On the work queue: reads everything again and shows it.
    private func load(bump: Bool) {
        guard let core else { return }
        let snapshot = Store.read(core)
        // Just connected: the catch-up lands a moment later, quietly when it
        // brings nothing new, so look again for its time.
        let connected = snapshot.state.online && !wasOnline
        wasOnline = snapshot.state.online
        DispatchQueue.main.async { self.apply(snapshot, bump: bump) }
        if connected {
            work.asyncAfter(deadline: .now() + 2) { [weak self] in self?.load(bump: true) }
        }
    }

    /// Reads the link's state and the lists again; "synced 2 min ago" moves
    /// on by itself this way.
    func refresh() {
        work.async { [weak self] in self?.load(bump: false) }
    }

    // MARK: Editing

    /// Runs an edit on the core off the main thread, then reads the lists
    /// again. A failure goes in the log as it came and on screen in plain words.
    func run(_ edit: @escaping (Core) throws -> Void) {
        guard let core else { return }
        work.async { [weak self] in
            do {
                try edit(core)
            } catch {
                self?.failed(coreWords(error))
            }
            self?.load(bump: true)
        }
    }

    /// Asks the core for something off the main thread and hands the answer
    /// back on it. With `reload`, the lists are read again first.
    func ask<Answer>(
        reload: Bool = false,
        _ question: @escaping (Core) throws -> Answer,
        then: @escaping (Result<Answer, Error>) -> Void
    ) {
        guard let core else {
            then(.failure(CoreError.Failed(message: failure ?? "Legato couldn't open its files.")))
            return
        }
        work.async { [weak self] in
            let answer = Result { try question(core) }
            if reload { self?.load(bump: true) }
            DispatchQueue.main.async { then(answer) }
        }
    }

    private func failed(_ raw: String) {
        heard(raw)
        DispatchQueue.main.async {
            self.show(plainWords(raw, pc: self.state.pcName))
        }
    }

    // MARK: The link

    /// The app came to the front: try the PC now, and keep the waits short.
    func foreground() {
        run { core in
            core.setScreenOn(on: true)
            if core.state().paired { try core.connect() }
        }
    }

    /// The app left the screen: iOS is about to stop it, so let the PC go
    /// cleanly rather than leave a link that looks open.
    func background() {
        guard let core else { return }
        work.async {
            core.setScreenOn(on: false)
            core.disconnect()
        }
    }

    /// Sync now; when the PC isn't connected, try to reach it and say so.
    func syncNow() {
        let pc = state.pcName ?? "your PC"
        run { [weak self] core in
            if try !core.syncNow() {
                try core.connect()
                self?.notice("Not connected to \(pc) right now. Trying again.")
            }
        }
    }

    private func pairingEnded(_ why: String) {
        heard("Unpaired: \(why)")
        DispatchQueue.main.async { self.ended = why }
    }

    // MARK: The log

    /// A line from the core, from whichever thread it spoke on.
    private func heard(_ line: String) {
        let entry = Store.stamp() + "  " + line
        DispatchQueue.main.async {
            self.lines.append(entry)
            if self.lines.count > Store.logLines {
                self.lines.removeFirst(self.lines.count - Store.logLines)
            }
        }
        work.async { [weak self] in
            guard let self else { return }
            Store.appendLog(entry, in: self.dir)
            self.load(bump: true)
        }
    }

    private static func stamp() -> String {
        let format = DateFormatter()
        format.locale = Locale(identifier: "en_US_POSIX")
        format.dateFormat = "MMM d HH:mm:ss"
        return format.string(from: Date())
    }

    private static func logFile(_ dir: URL) -> URL {
        dir.appendingPathComponent("log.txt")
    }

    /// The last lines of the log; a file grown past twice that is cut back.
    private static func readLog(_ dir: URL) -> [String] {
        guard let text = try? String(contentsOf: logFile(dir), encoding: .utf8) else { return [] }
        let all = text.split(separator: "\n").map(String.init)
        let kept = Array(all.suffix(logLines))
        if all.count > logLines * 2 {
            try? (kept.joined(separator: "\n") + "\n").write(to: logFile(dir), atomically: true, encoding: .utf8)
        }
        return kept
    }

    private static func appendLog(_ entry: String, in dir: URL) {
        let file = logFile(dir)
        let data = Data((entry + "\n").utf8)
        if let handle = try? FileHandle(forWritingTo: file) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            try? data.write(to: file)
        }
    }

    // MARK: Toasts

    /// Shows `text` at the bottom of the screen for a moment. Main thread.
    func show(_ text: String) {
        present(Toast(text: text), for: 2.5)
    }

    /// Shows `text` with an action (Undo) for a few seconds. Main thread.
    func show(_ text: String, action: String, perform: @escaping () -> Void) {
        present(Toast(text: text, actionLabel: action, action: perform), for: 6)
    }

    /// As above, with something to do once the toast has gone untapped:
    /// a delete that waits for Undo to pass. Main thread.
    func show(_ text: String, action: String, expired: @escaping () -> Void, perform: @escaping () -> Void) {
        present(Toast(text: text, actionLabel: action, action: perform, expired: expired), for: 6)
    }

    /// Shows `text` from any thread.
    func notice(_ text: String) {
        DispatchQueue.main.async { self.show(text) }
    }

    /// The toast's action was tapped.
    func toastTapped() {
        guard let current = toast else { return }
        toast = nil
        current.action?()
    }

    private func present(_ next: Toast, for seconds: TimeInterval) {
        // A toast that makes way for another has run out of time.
        if let old = toast { old.expired?() }
        toast = next
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { [weak self] in
            guard let self, self.toast?.id == next.id else { return }
            self.toast = nil
            next.expired?()
        }
    }

    // MARK: Files

    private static func home() -> URL {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return support.appendingPathComponent("phone", isDirectory: true)
    }

    /// Makes the folder and keeps it out of backups: this phone's key and
    /// its pairing don't move to another phone, and the PC has the rest.
    private static func prepare(_ dir: URL) {
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        var folder = dir
        try? folder.setResourceValues(values)
    }
}
