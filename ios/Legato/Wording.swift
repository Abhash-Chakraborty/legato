import Foundation
import LegatoCore

// The words Legato for iPhone shares with Legato for Android, so the two
// phones read the same: times, the PC's status, a Scratchpad note's heading.

/// "just now", "5 min ago", "3 h ago", "2 d ago".
func ago(_ at: Int64, now: Date = Date()) -> String {
    let seconds = Int64(now.timeIntervalSince1970) - at
    if seconds < 60 { return "just now" }
    if seconds < 3600 { return "\(seconds / 60) min ago" }
    if seconds < 86400 { return "\(seconds / 3600) h ago" }
    return "\(seconds / 86400) d ago"
}

/// "Today", "Yesterday", or the date. Midnights are compared, so January 1
/// still has a yesterday.
func dayLabel(_ at: Int64, now: Date = Date(), calendar: Calendar = .current) -> String {
    let then = Date(timeIntervalSince1970: TimeInterval(at))
    let today = calendar.startOfDay(for: now)
    let day = calendar.startOfDay(for: then)
    if day == today { return "Today" }
    if let yesterday = calendar.date(byAdding: .day, value: -1, to: today), day == yesterday {
        return "Yesterday"
    }
    return then.formatted(date: .abbreviated, time: .omitted)
}

func timeOf(_ at: Int64) -> String {
    Date(timeIntervalSince1970: TimeInterval(at)).formatted(date: .omitted, time: .shortened)
}

func dateAndTime(_ at: Int64) -> String {
    Date(timeIntervalSince1970: TimeInterval(at)).formatted(date: .abbreviated, time: .shortened)
}

/// How long a recording ran: "under a minute", "24 min", "1 h 5 min".
func minutes(_ ms: Int64) -> String {
    let m = ms / 60_000
    if m < 1 { return "under a minute" }
    if m < 60 { return "\(m) min" }
    return "\(m / 60) h \(m % 60) min"
}

func openActions(_ count: UInt32) -> String? {
    switch count {
    case 0: return nil
    case 1: return "1 open action"
    default: return "\(count) open actions"
    }
}

/// The parts that are there, joined with a middle dot.
func dotted(_ parts: [String?]) -> String {
    parts.compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
}

// MARK: - The link to the PC

/// How the link stands: each has its own colour and its own words.
enum LinkMood: Equatable {
    /// Not paired.
    case off
    case connected
    /// Paired, out of reach, and Legato keeps trying.
    case trying
    /// The PC ended or refused the pairing.
    case ended
}

struct PcStatus: Equatable {
    let mood: LinkMood
    let text: String
}

/// How the phone reaches the PC right now, in words.
func connectedText(_ via: String?) -> String {
    switch via ?? "" {
    case "bluetooth": return "Connected over Bluetooth"
    case "netbird": return "Connected through your VPN"
    default: return "Connected on your network"
    }
}

/// The PC's status for the chip on Home (short) and for Settings (long).
func pcStatus(_ state: PhoneState, device: String, long: Bool = false, now: Date = Date()) -> PcStatus {
    guard state.paired else { return PcStatus(mood: .off, text: "Not paired") }
    let pc = state.pcName ?? "Your PC"
    let synced = state.lastSync.map { ago($0, now: now) }
    if state.online {
        if long {
            return PcStatus(mood: .connected, text: connectedText(state.via) + (synced.map { " · synced \($0)" } ?? ""))
        }
        return PcStatus(mood: .connected, text: "\(pc) · " + (synced.map { "synced \($0)" } ?? "connected"))
    }
    if long {
        let last = synced.map { " · last synced \($0)" } ?? ""
        return PcStatus(mood: .trying, text: "Can't reach it right now · changes wait on this \(device)" + last)
    }
    return PcStatus(mood: .trying, text: "\(pc) not reachable · changes wait here")
}

/// A failure from the core in words for the screen; the log keeps it as it came.
func plainWords(_ raw: String, pc: String?) -> String {
    let raw = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    let low = raw.lowercased()
    let name = pc ?? "your PC"
    func has(_ needles: [String]) -> Bool { needles.contains { low.contains($0) } }
    if raw.isEmpty { return "That didn't work. Try again." }
    if has(["timed out", "timeout", "unreachable", "refused", "reset", "broken pipe", "no route", "not connected", "closed"]) {
        return "Couldn't reach \(name). Check it's on, Legato is running there, and this device is on its network."
    }
    if has(["database is locked", "busy"]) { return "Legato was busy for a moment. Try again." }
    if has(["no such table", "sqlite", "constraint"]) {
        return "Legato couldn't save that. Try again, and restart the app if it keeps happening."
    }
    if has(["certificate", "tls", "handshake"]) {
        return "\(name) didn't recognise this device. Pair again from Settings › Devices on the PC."
    }
    // The core words its own problems for people already.
    if raw.count <= 140, !raw.contains("::"), !raw.contains("Error(") { return raw }
    return "That didn't work. Try again."
}

// MARK: - Scratchpad notes

/// A note's heading: its title, else its first line up to 80 characters.
func scratchHeading(title: String, text: String) -> String {
    if !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { return title }
    let first = text
        .split(whereSeparator: \.isNewline)
        .map { $0.trimmingCharacters(in: .whitespaces) }
        .first { !$0.isEmpty }
    let heading = first.map(clipLine) ?? ""
    return heading.isEmpty ? "Untitled" : heading
}

/// The first 80 characters of a line, ending on a whole word.
func clipLine(_ line: String) -> String {
    guard line.count > 80 else { return line }
    var cut = String(line.prefix(80))
    if let space = cut.lastIndex(of: " "), cut.distance(from: cut.startIndex, to: space) > 40 {
        cut = String(cut[..<space])
    }
    while let last = cut.last, ",;: ".contains(last) { cut.removeLast() }
    return cut + "…"
}

/// The text under the heading, without what the heading already shows.
/// A title "Ok" over "Okay, …" isn't the start of it: only a whole word or
/// a heading cut short counts.
func scratchPreview(title: String, text: String) -> String {
    let body = text.trimmingCharacters(in: .whitespacesAndNewlines)
    var head = scratchHeading(title: title, text: text)
    let cutShort = head.hasSuffix("…")
    if cutShort { head.removeLast() }
    var shown = body
    if !head.isEmpty, body.hasPrefix(head) {
        let rest = String(body.dropFirst(head.count))
        let atWord: Bool
        if let next = rest.first {
            atWord = cutShort || !(next.isLetter || next.isNumber)
        } else {
            atWord = true
        }
        if atWord { shown = rest }
    }
    let trimmed = shown.drop { ",;: \n".contains($0) }
    return trimmed.split(whereSeparator: \.isWhitespace).joined(separator: " ")
}

// MARK: - Lists by day

struct Day<Item>: Identifiable {
    let label: String
    var items: [Item]
    var id: String { label }
}

/// Items already in time order, grouped under "Today", "Yesterday" and dates.
func byDay<Item>(_ items: [Item], at: (Item) -> Int64) -> [Day<Item>] {
    var days: [Day<Item>] = []
    for item in items {
        let label = dayLabel(at(item))
        if let last = days.indices.last, days[last].label == label {
            days[last].items.append(item)
        } else {
            days.append(Day(label: label, items: [item]))
        }
    }
    return days
}
