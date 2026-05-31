use anyhow::Result;

use crate::output::styling::Styler;

pub const AUTHOR: &str = "Cuma Kurt <cumakurt@gmail.com>";
pub const GITHUB: &str = "https://github.com/cumakurt/mh";
pub const LINKEDIN: &str = "https://www.linkedin.com/in/cuma-kurt-34414917/";

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
    }
}
