//! Which microphone to record from: your pick, else the next one in your
//! order that's plugged in. Free of Tauri so it can be tested on any OS.

/// How many microphones the order remembers.
pub const ORDER_LEN: usize = 8;

/// The microphone to use from those plugged in (`present`, by name): your
/// pick, else the first of your order that's there. None leaves it to
/// Windows' default, as does choosing Default (`selected` None).
pub fn pick_microphone(
    selected: Option<&str>,
    order: &[String],
    present: &[&str],
) -> Option<String> {
    let selected = selected?;
    std::iter::once(selected)
        .chain(order.iter().map(String::as_str))
        .find(|name| present.contains(name))
        .map(str::to_string)
}

/// The order after you pick a microphone: it goes first, and the order
/// keeps the last `ORDER_LEN`.
pub fn ranked_first(order: &[String], name: &str) -> Vec<String> {
    std::iter::once(name.to_string())
        .chain(order.iter().filter(|n| *n != name).cloned())
        .take(ORDER_LEN)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn the_first_microphone_plugged_in_wins() {
        let order = names(&["Jabra Evolve2", "Yeti", "Laptop array"]);
        // The headset is there: it's used.
        assert_eq!(
            pick_microphone(
                Some("Jabra Evolve2"),
                &order,
                &["Laptop array", "Jabra Evolve2"]
            ),
            Some("Jabra Evolve2".into())
        );
        // Unplugged: the next one in the order, not Windows' default.
        assert_eq!(
            pick_microphone(Some("Jabra Evolve2"), &order, &["Laptop array", "Yeti"]),
            Some("Yeti".into())
        );
        // None of them: Windows' default.
        assert_eq!(
            pick_microphone(Some("Jabra Evolve2"), &order, &["USB Audio"]),
            None
        );
        // Default chosen: Windows' default, whatever the order.
        assert_eq!(pick_microphone(None, &order, &["Yeti"]), None);
    }

    #[test]
    fn a_picked_microphone_goes_first() {
        let order = names(&["Yeti", "Laptop array"]);
        assert_eq!(
            ranked_first(&order, "Laptop array"),
            names(&["Laptop array", "Yeti"])
        );
        assert_eq!(
            ranked_first(&order, "Jabra"),
            names(&["Jabra", "Yeti", "Laptop array"])
        );
        let long: Vec<String> = (0..10).map(|i| format!("Mic {i}")).collect();
        assert_eq!(ranked_first(&long, "New").len(), ORDER_LEN);
    }
}
