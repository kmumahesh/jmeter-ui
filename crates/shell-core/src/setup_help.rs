//! What the welcome screen tells a person when Java or JMeter is missing: the words and the places to get them. The
//! words are kept here, away from the window, so they can be checked and kept the same as the README.

/// The JMeter version the app is tested with.
pub const TESTED_JMETER: &str = "5.6.3";

/// The oldest Java the app asks for.
pub const MINIMUM_JAVA: u32 = 17;

/// A page to open in the browser.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub label: &'static str,
    pub url: &'static str,
}

/// What to show for one missing thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Help {
    /// The headline: what was not found.
    pub what: &'static str,
    /// What to do about it, in plain words.
    pub text: String,
    /// Pages that help, each opened in the browser.
    pub links: Vec<Link>,
}

pub const JAVA_MISSING: &str = "Java was not found";
pub const JMETER_MISSING: &str = "JMeter was not found";

/// Where to get a Java runtime (Eclipse Temurin, which is free to use and redistribute).
pub const JAVA_URL: &str = "https://adoptium.net/temurin/releases/";
/// The official page for downloading Apache JMeter (the newest release).
pub const JMETER_URL: &str = "https://jmeter.apache.org/download_jmeter.cgi";
/// The JMeter version the app is tested with, from Apache's archive of releases.
pub const JMETER_TESTED_URL: &str = "https://archive.apache.org/dist/jmeter/binaries/apache-jmeter-5.6.3.zip";

/// Every address the welcome screen can open. Only these are ever opened.
pub const LINKS: [&str; 3] = [JAVA_URL, JMETER_URL, JMETER_TESTED_URL];

/// What to say when Java is missing (`java_missing`) or when only JMeter is.
pub fn help(java_missing: bool) -> Help {
    if java_missing {
        Help {
            what: JAVA_MISSING,
            text: format!(
                "JMeter needs Java {MINIMUM_JAVA} or newer. A Java runtime (JRE) is enough; you do not need the full JDK. Install one, then press Check again. If Java is installed somewhere unusual, choose it in Settings."
            ),
            links: vec![Link { label: "Get Java (Eclipse Temurin)", url: JAVA_URL }],
        }
    } else {
        Help {
            what: JMETER_MISSING,
            text: format!(
                "Download Apache JMeter (the binary .zip; this app is tested with {TESTED_JMETER}) and unpack it. If you unpack it in one of the usual places, the app finds it by itself. Otherwise choose its folder in Settings. Then press Check again."
            ),
            links: vec![Link { label: "Apache JMeter downloads", url: JMETER_URL }, Link { label: "JMeter 5.6.3 (tested)", url: JMETER_TESTED_URL }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn both() -> [Help; 2] {
        [help(true), help(false)]
    }

    #[test]
    fn each_missing_thing_says_what_to_do_and_where_to_get_it() {
        let java = help(true);
        assert_eq!(java.what, JAVA_MISSING);
        for word in ["runtime", "JRE", "JDK", "Check again", "Settings", "17"] {
            assert!(java.text.contains(word), "{word}: {}", java.text);
        }
        assert_eq!(java.links.len(), 1);
        let jmeter = help(false);
        assert_eq!(jmeter.what, JMETER_MISSING);
        for word in ["5.6.3", "Check again", "Settings", "usual places", ".zip"] {
            assert!(jmeter.text.contains(word), "{word}: {}", jmeter.text);
        }
        assert_eq!(jmeter.links.len(), 2);
    }

    #[test]
    fn every_link_is_a_secure_address_on_the_expected_site_and_listed() {
        let hosts = ["https://adoptium.net/", "https://jmeter.apache.org/", "https://archive.apache.org/"];
        for h in both() {
            for l in &h.links {
                assert!(l.url.starts_with("https://") && !l.url.contains(' '), "{l:?}");
                assert!(hosts.iter().any(|p| l.url.starts_with(p)), "{l:?} is not on a site the welcome screen trusts");
                assert!(LINKS.contains(&l.url), "{l:?} is not in the list of addresses that may be opened");
                assert!(!l.label.is_empty());
            }
        }
        assert!(LINKS.iter().all(|u| u.starts_with("https://")));
    }

    #[test]
    fn the_words_do_not_depend_on_one_computer() {
        for h in both() {
            let all = format!("{} {} {:?}", h.what, h.text, h.links);
            for bad in ["Mahesh", "C:\\", "C:/", "/home/", "/Users/", "Downloads", "D:\\"] {
                assert!(!all.contains(bad), "{bad} in: {all}");
            }
        }
    }

    #[test]
    fn the_tested_version_matches_the_readme_and_the_tested_link() {
        let readme = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md")).unwrap();
        assert!(readme.contains(&format!("JMeter {TESTED_JMETER}")), "the README names the tested version");
        assert!(JMETER_TESTED_URL.contains(&format!("apache-jmeter-{TESTED_JMETER}.zip")));
        assert!(readme.contains(&format!("Java {MINIMUM_JAVA} or newer")), "the README names the oldest Java");
    }
}
