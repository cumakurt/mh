use anyhow::Result;

use crate::output::styling::Styler;

pub const AUTHOR: &str = "Cuma Kurt <cumakurt@gmail.com>";
pub const GITHUB: &str = "https://github.com/cumakurt/mh";
pub const LINKEDIN: &str = "https://www.linkedin.com/in/cuma-kurt-34414917/";
pub const LICENSE: &str = "AGPL-3.0-or-later";
pub const LICENSE_URL: &str = "https://www.gnu.org/licenses/agpl-3.0.html";

pub fn run() -> Result<()> {
    let config = crate::config::AppConfig::load()?;
    let styler = Styler::from_config(&config);

    println!(
        "{} {}",
        styler.accent("mh"),
        styler.success(env!("CARGO_PKG_VERSION"))
    );
    println!("Author: {}", styler.accent(AUTHOR));
    println!("GitHub: {}", styler.accent(GITHUB));
    println!("LinkedIn: {}", styler.accent(LINKEDIN));
    println!(
        "License: {} ({})",
        styler.warning(LICENSE),
        styler.accent(LICENSE_URL)
    );
    println!(
        "{}",
        styler.muted(
            "Distributed under the GNU Affero GPL v3+. Network use may require source offer."
        )
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn developer_metadata_is_available() {
        assert_eq!(AUTHOR, "Cuma Kurt <cumakurt@gmail.com>");
        assert_eq!(GITHUB, "https://github.com/cumakurt/mh");
        assert_eq!(LINKEDIN, "https://www.linkedin.com/in/cuma-kurt-34414917/");
        assert_eq!(LICENSE, "AGPL-3.0-or-later");
    }
}
