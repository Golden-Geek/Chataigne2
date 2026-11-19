// Reserve space for the SDK's machine prefix, also for multibyte local names.
const MAX_LOCAL_NAME_BYTES: usize = 180;

fn allowed(character: char) -> bool {
    !character.is_control() && !matches!(character, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

/// Is this local name valid before the SDK adds its host prefix?
pub fn valid_ndi_sender_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= MAX_LOCAL_NAME_BYTES && name.chars().all(allowed)
}

/// Normalize a derived sender name before duplicate checks and SDK creation.
pub fn normalize_ndi_sender_name(name: &str) -> String {
    let mut result = String::new();
    for character in name.trim().chars() {
        let character = if allowed(character) { character } else { ' ' };
        if result.len() + character.len_utf8() > MAX_LOCAL_NAME_BYTES {
            break;
        }
        result.push(character);
    }
    result.truncate(result.trim_end().len());
    result
}
