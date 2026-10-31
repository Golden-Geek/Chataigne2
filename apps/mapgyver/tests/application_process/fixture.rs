use std::time::Duration;

fn main() {
    // Finite, headless owned target. The parent test must stop it before this fallback deadline.
    std::thread::sleep(Duration::from_secs(20));
}
