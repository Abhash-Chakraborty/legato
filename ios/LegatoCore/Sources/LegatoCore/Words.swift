import Foundation

// LegatoCore.swift, beside this file, is written by UniFFI from the Rust
// core (ios/scripts/build-core.sh). This file adds what the generated code
// leaves out.

/// What went wrong, as the core worded it: the generated error's own
/// description wraps the line in the enum's name.
public func coreWords(_ error: Error) -> String {
    if let error = error as? CoreError {
        switch error {
        case let .Failed(message):
            return message
        }
    }
    return error.localizedDescription
}
