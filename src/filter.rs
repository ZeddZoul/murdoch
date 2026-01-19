//! Layer 1: Regex-based content filtering.
//!
//! Performs instant pattern matching against configurable regex patterns
//! for slurs, invite links, and phishing URLs.

use std::sync::{Arc, RwLock};

use regex::RegexSet;

use crate::error::{MurdochError, Result};
use crate::models::{FilterResult, PatternType};

/// A set of compiled regex patterns for each category.
pub struct PatternSet {
    pub slurs: RegexSet,
    pub invite_links: RegexSet,
    pub phishing_urls: RegexSet,
}

impl PatternSet {
    /// Create a new PatternSet from string patterns.
    ///
    /// Returns an error if any pattern fails to compile.
    pub fn new(
        slurs: &[String],
        invite_links: &[String],
        phishing_urls: &[String],
    ) -> Result<Self> {
        Ok(Self {
            slurs: RegexSet::new(slurs)?,
            invite_links: RegexSet::new(invite_links)?,
            phishing_urls: RegexSet::new(phishing_urls)?,
        })
    }

    /// Create an empty PatternSet (matches nothing).
    pub fn empty() -> Result<Self> {
        Self::new(&[], &[], &[])
    }
}

/// Layer 1 regex filter with runtime-configurable patterns.
pub struct RegexFilter {
    patterns: Arc<RwLock<PatternSet>>,
}

impl RegexFilter {
    /// Create a new RegexFilter with the given patterns.
    pub fn new(patterns: PatternSet) -> Self {
        Self {
            patterns: Arc::new(RwLock::new(patterns)),
        }
    }

    /// Evaluate a message against all configured patterns.
    ///
    /// Returns `FilterResult::Violation` if any pattern matches,
    /// or `FilterResult::Pass` if no patterns match.
    pub fn evaluate(&self, content: &str) -> FilterResult {
        let patterns = self
            .patterns
            .read()
            .map_err(|_| MurdochError::InternalState("lock poisoned".to_string()))
            .expect("lock should not be poisoned in tests");

        // Check slurs first (highest priority)
        if patterns.slurs.is_match(content) {
            return FilterResult::Violation {
                reason: "Matched slur pattern".to_string(),
                pattern_type: PatternType::Slur,
            };
        }

        // Check invite links
        if patterns.invite_links.is_match(content) {
            return FilterResult::Violation {
                reason: "Matched invite link pattern".to_string(),
                pattern_type: PatternType::InviteLink,
            };
        }

        // Check phishing URLs
        if patterns.phishing_urls.is_match(content) {
            return FilterResult::Violation {
                reason: "Matched phishing URL pattern".to_string(),
                pattern_type: PatternType::PhishingUrl,
            };
        }

        FilterResult::Pass
    }

    /// Evaluate without panicking on lock errors.
    pub fn try_evaluate(&self, content: &str) -> Result<FilterResult> {
        let patterns = self
            .patterns
            .read()
            .map_err(|_| MurdochError::InternalState("lock poisoned".to_string()))?;

        if patterns.slurs.is_match(content) {
            return Ok(FilterResult::Violation {
                reason: "Matched slur pattern".to_string(),
                pattern_type: PatternType::Slur,
            });
        }

        if patterns.invite_links.is_match(content) {
            return Ok(FilterResult::Violation {
                reason: "Matched invite link pattern".to_string(),
                pattern_type: PatternType::InviteLink,
            });
        }

        if patterns.phishing_urls.is_match(content) {
            return Ok(FilterResult::Violation {
                reason: "Matched phishing URL pattern".to_string(),
                pattern_type: PatternType::PhishingUrl,
            });
        }

        Ok(FilterResult::Pass)
    }

    /// Update patterns at runtime without restart.
    ///
    /// The new patterns are validated before being applied.
    pub fn update_patterns(&self, new_patterns: PatternSet) -> Result<()> {
        let mut patterns = self
            .patterns
            .write()
            .map_err(|_| MurdochError::InternalState("lock poisoned".to_string()))?;

        *patterns = new_patterns;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::filter::{PatternSet, RegexFilter};
    use crate::models::{FilterResult, PatternType};

    fn test_patterns() -> PatternSet {
        PatternSet::new(
            &["badword".to_string(), "slur\\d+".to_string()],
            &["discord\\.gg/".to_string(), "invite\\.link".to_string()],
            &["phish\\.com".to_string(), "scam\\.net".to_string()],
        )
        .expect("test patterns should compile")
    }

    #[test]
    fn evaluate_slur_match() {
        let filter = RegexFilter::new(test_patterns());
        let result = filter.evaluate("this contains badword here");

        assert!(matches!(
            result,
            FilterResult::Violation {
                pattern_type: PatternType::Slur,
                ..
            }
        ));
    }

    #[test]
    fn evaluate_slur_regex_match() {
        let filter = RegexFilter::new(test_patterns());
        let result = filter.evaluate("slur123 is bad");

        assert!(matches!(
            result,
            FilterResult::Violation {
                pattern_type: PatternType::Slur,
                ..
            }
        ));
    }

    #[test]
    fn evaluate_invite_link_match() {
        let filter = RegexFilter::new(test_patterns());
        let result = filter.evaluate("join us at discord.gg/abc123");

        assert!(matches!(
            result,
            FilterResult::Violation {
                pattern_type: PatternType::InviteLink,
                ..
            }
        ));
    }

    #[test]
    fn evaluate_phishing_match() {
        let filter = RegexFilter::new(test_patterns());
        let result = filter.evaluate("click here: http://phish.com/login");

        assert!(matches!(
            result,
            FilterResult::Violation {
                pattern_type: PatternType::PhishingUrl,
                ..
            }
        ));
    }

    #[test]
    fn evaluate_clean_message_passes() {
        let filter = RegexFilter::new(test_patterns());
        let result = filter.evaluate("hello, this is a normal message");

        assert_eq!(result, FilterResult::Pass);
    }

    #[test]
    fn update_patterns_runtime() {
        let filter = RegexFilter::new(PatternSet::empty().expect("empty should work"));

        // Initially passes
        assert_eq!(filter.evaluate("newbadword"), FilterResult::Pass);

        // Update patterns
        let new_patterns = PatternSet::new(&["newbadword".to_string()], &[], &[])
            .expect("new patterns should compile");
        filter
            .update_patterns(new_patterns)
            .expect("update should succeed");

        // Now matches
        assert!(matches!(
            filter.evaluate("newbadword"),
            FilterResult::Violation {
                pattern_type: PatternType::Slur,
                ..
            }
        ));
    }

    #[test]
    fn empty_patterns_pass_everything() {
        let filter = RegexFilter::new(PatternSet::empty().expect("empty should work"));

        assert_eq!(filter.evaluate("anything goes"), FilterResult::Pass);
        assert_eq!(filter.evaluate("discord.gg/test"), FilterResult::Pass);
    }
}

#[cfg(test)]
mod property_tests {
    use crate::filter::{PatternSet, RegexFilter};
    use crate::models::{FilterResult, PatternType};
    use proptest::prelude::*;

    /// Generate a random string that contains a specific pattern.
    fn string_containing(pattern: &str) -> impl Strategy<Value = String> {
        let pattern = pattern.to_string();
        ("[a-z ]{0,20}", "[a-z ]{0,20}")
            .prop_map(move |(prefix, suffix)| format!("{}{}{}", prefix, pattern, suffix))
    }

    /// Generate a random string that does NOT contain any of the test patterns.
    fn clean_string() -> impl Strategy<Value = String> {
        // Generate strings from a safe alphabet that won't match our patterns
        "[a-z ]{1,50}".prop_filter("must not contain patterns", |s| {
            !s.contains("badword")
                && !s.contains("discord.gg")
                && !s.contains("phish.com")
                && !s.contains("slur")
        })
    }

    fn test_patterns() -> PatternSet {
        PatternSet::new(
            &["badword".to_string()],
            &["discord\\.gg".to_string()],
            &["phish\\.com".to_string()],
        )
        .expect("test patterns should compile")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// **Feature: murdoch-discord-bot, Property 1: Pattern Matching Flags Violations**
        /// **Validates: Requirements 1.2, 1.3, 1.4**
        ///
        /// For any message content containing a configured pattern,
        /// the RegexFilter SHALL return a Violation result with the correct pattern type.
        #[test]
        fn prop_slur_pattern_flags_violation(content in string_containing("badword")) {
            let filter = RegexFilter::new(test_patterns());
            let result = filter.evaluate(&content);

            prop_assert!(
                matches!(result, FilterResult::Violation { pattern_type: PatternType::Slur, .. }),
                "Content '{}' should match slur pattern, got {:?}", content, result
            );
        }

        #[test]
        fn prop_invite_pattern_flags_violation(content in string_containing("discord.gg")) {
            let filter = RegexFilter::new(test_patterns());
            let result = filter.evaluate(&content);

            prop_assert!(
                matches!(result, FilterResult::Violation { pattern_type: PatternType::InviteLink, .. }),
                "Content '{}' should match invite pattern, got {:?}", content, result
            );
        }

        #[test]
        fn prop_phishing_pattern_flags_violation(content in string_containing("phish.com")) {
            let filter = RegexFilter::new(test_patterns());
            let result = filter.evaluate(&content);

            prop_assert!(
                matches!(result, FilterResult::Violation { pattern_type: PatternType::PhishingUrl, .. }),
                "Content '{}' should match phishing pattern, got {:?}", content, result
            );
        }

        /// **Feature: murdoch-discord-bot, Property 2: Non-Matching Messages Pass Through**
        /// **Validates: Requirements 1.5**
        ///
        /// For any message content that does not match any configured pattern,
        /// the RegexFilter SHALL return a Pass result.
        #[test]
        fn prop_clean_messages_pass(content in clean_string()) {
            let filter = RegexFilter::new(test_patterns());
            let result = filter.evaluate(&content);

            prop_assert!(
                result == FilterResult::Pass,
                "Clean content '{}' should pass, got {:?}", content, result
            );
        }

        /// **Feature: murdoch-discord-bot, Property 3: Runtime Pattern Updates Take Effect**
        /// **Validates: Requirements 1.6**
        ///
        /// For any pattern update operation, subsequent evaluations SHALL use
        /// the updated patterns without requiring system restart.
        #[test]
        fn prop_runtime_pattern_updates(
            new_pattern in "[a-z]{3,10}",
            test_content in "[a-z ]{5,30}"
        ) {
            // Start with empty patterns
            let filter = RegexFilter::new(PatternSet::empty().expect("empty should work"));

            // Content should pass initially
            prop_assert_eq!(filter.evaluate(&test_content), FilterResult::Pass);

            // Update with new pattern
            let content_with_pattern = format!("{} {}", test_content, new_pattern);
            let new_patterns = PatternSet::new(std::slice::from_ref(&new_pattern), &[], &[])
                .expect("pattern should compile");
            filter.update_patterns(new_patterns).expect("update should succeed");

            // Content containing new pattern should now be flagged
            let result = filter.evaluate(&content_with_pattern);
            prop_assert!(
                matches!(result, FilterResult::Violation { pattern_type: PatternType::Slur, .. }),
                "Content '{}' should match new pattern '{}', got {:?}",
                content_with_pattern, new_pattern, result
            );
        }
    }
}
