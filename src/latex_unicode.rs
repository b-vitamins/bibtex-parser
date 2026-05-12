//! LaTeX to Unicode conversion for common escape sequences
//!
//! This module provides conversion from LaTeX escape sequences to Unicode
//! characters for improved readability of BibTeX data.

use phf::phf_map;

/// Common LaTeX accent commands to Unicode (direct format like \'e)
static LATEX_ACCENTS: phf::Map<&'static str, &'static str> = phf_map! {
    // Acute accent: \' (both single and double backslash versions)
    "\\'a" => "á", "\\\\'a" => "á",
    "\\'e" => "é", "\\\\'e" => "é",
    "\\'i" => "í", "\\\\'i" => "í",
    "\\'o" => "ó", "\\\\'o" => "ó",
    "\\'u" => "ú", "\\\\'u" => "ú",
    "\\'A" => "Á", "\\\\'A" => "Á",
    "\\'E" => "É", "\\\\'E" => "É",
    "\\'I" => "Í", "\\\\'I" => "Í",
    "\\'O" => "Ó", "\\\\'O" => "Ó",
    "\\'U" => "Ú", "\\\\'U" => "Ú",
    "\\'y" => "ý", "\\\\'y" => "ý",
    "\\'Y" => "Ý", "\\\\'Y" => "Ý",

    // Grave accent: \` (both single and double backslash versions)
    "\\`a" => "à", "\\\\`a" => "à",
    "\\`e" => "è", "\\\\`e" => "è",
    "\\`i" => "ì", "\\\\`i" => "ì",
    "\\`o" => "ò", "\\\\`o" => "ò",
    "\\`u" => "ù", "\\\\`u" => "ù",
    "\\`A" => "À", "\\\\`A" => "À",
    "\\`E" => "È", "\\\\`E" => "È",
    "\\`I" => "Ì", "\\\\`I" => "Ì",
    "\\`O" => "Ò", "\\\\`O" => "Ò",
    "\\`U" => "Ù", "\\\\`U" => "Ù",

    // Circumflex: \^ (both single and double backslash versions)
    "\\^a" => "â", "\\\\^a" => "â",
    "\\^e" => "ê", "\\\\^e" => "ê",
    "\\^i" => "î", "\\\\^i" => "î",
    "\\^o" => "ô", "\\\\^o" => "ô",
    "\\^u" => "û", "\\\\^u" => "û",
    "\\^A" => "Â", "\\\\^A" => "Â",
    "\\^E" => "Ê", "\\\\^E" => "Ê",
    "\\^I" => "Î", "\\\\^I" => "Î",
    "\\^O" => "Ô", "\\\\^O" => "Ô",
    "\\^U" => "Û", "\\\\^U" => "Û",

    // Umlaut/Diaeresis: \" (single, double, and triple backslash versions)
    "\\\"a" => "ä", "\\\\\"a" => "ä", "\\\\\\\"a" => "ä",
    "\\\"e" => "ë", "\\\\\"e" => "ë", "\\\\\\\"e" => "ë",
    "\\\"i" => "ï", "\\\\\"i" => "ï", "\\\\\\\"i" => "ï",
    "\\\"o" => "ö", "\\\\\"o" => "ö", "\\\\\\\"o" => "ö",
    "\\\"u" => "ü", "\\\\\"u" => "ü", "\\\\\\\"u" => "ü",
    "\\\"A" => "Ä", "\\\\\"A" => "Ä", "\\\\\\\"A" => "Ä",
    "\\\"E" => "Ë", "\\\\\"E" => "Ë", "\\\\\\\"E" => "Ë",
    "\\\"I" => "Ï", "\\\\\"I" => "Ï", "\\\\\\\"I" => "Ï",
    "\\\"O" => "Ö", "\\\\\"O" => "Ö", "\\\\\\\"O" => "Ö",
    "\\\"U" => "Ü", "\\\\\"U" => "Ü", "\\\\\\\"U" => "Ü",
    "\\\"y" => "ÿ", "\\\\\"y" => "ÿ", "\\\\\\\"y" => "ÿ",
    "\\\"Y" => "Ÿ", "\\\\\"Y" => "Ÿ", "\\\\\\\"Y" => "Ÿ",

    // Tilde: \~ (both single and double backslash versions)
    "\\~a" => "ã", "\\\\~a" => "ã",
    "\\~n" => "ñ", "\\\\~n" => "ñ",
    "\\~o" => "õ", "\\\\~o" => "õ",
    "\\~A" => "Ã", "\\\\~A" => "Ã",
    "\\~N" => "Ñ", "\\\\~N" => "Ñ",
    "\\~O" => "Õ", "\\\\~O" => "Õ",

    // Cedilla: \c with space (both single and double backslash versions)
    "\\c c" => "ç", "\\\\c c" => "ç",
    "\\c C" => "Ç", "\\\\c C" => "Ç",

    // Ring: \r with space (both single and double backslash versions)
    "\\r a" => "å", "\\\\r a" => "å",
    "\\r A" => "Å", "\\\\r A" => "Å",
};

/// LaTeX commands with braces like \'{e}, \"{o}, etc.
static LATEX_BRACED: phf::Map<&'static str, &'static str> = phf_map! {
    // Acute accent (both single and double backslash versions)
    "\\'{a}" => "á", "\\\\'{a}" => "á",
    "\\'{e}" => "é", "\\\\'{e}" => "é",
    "\\'{i}" => "í", "\\\\'{i}" => "í",
    "\\'{o}" => "ó", "\\\\'{o}" => "ó",
    "\\'{u}" => "ú", "\\\\'{u}" => "ú",
    "\\'{A}" => "Á", "\\\\'{A}" => "Á",
    "\\'{E}" => "É", "\\\\'{E}" => "É",
    "\\'{I}" => "Í", "\\\\'{I}" => "Í",
    "\\'{O}" => "Ó", "\\\\'{O}" => "Ó",
    "\\'{U}" => "Ú", "\\\\'{U}" => "Ú",
    "\\'{y}" => "ý", "\\\\'{y}" => "ý",
    "\\'{Y}" => "Ý", "\\\\'{Y}" => "Ý",

    // Grave accent (both single and double backslash versions)
    "\\`{a}" => "à", "\\\\`{a}" => "à",
    "\\`{e}" => "è", "\\\\`{e}" => "è",
    "\\`{i}" => "ì", "\\\\`{i}" => "ì",
    "\\`{o}" => "ò", "\\\\`{o}" => "ò",
    "\\`{u}" => "ù", "\\\\`{u}" => "ù",
    "\\`{A}" => "À", "\\\\`{A}" => "À",
    "\\`{E}" => "È", "\\\\`{E}" => "È",
    "\\`{I}" => "Ì", "\\\\`{I}" => "Ì",
    "\\`{O}" => "Ò", "\\\\`{O}" => "Ò",
    "\\`{U}" => "Ù", "\\\\`{U}" => "Ù",

    // Circumflex (both single and double backslash versions)
    "\\^{a}" => "â", "\\\\^{a}" => "â",
    "\\^{e}" => "ê", "\\\\^{e}" => "ê",
    "\\^{i}" => "î", "\\\\^{i}" => "î",
    "\\^{o}" => "ô", "\\\\^{o}" => "ô",
    "\\^{u}" => "û", "\\\\^{u}" => "û",
    "\\^{A}" => "Â", "\\\\^{A}" => "Â",
    "\\^{E}" => "Ê", "\\\\^{E}" => "Ê",
    "\\^{I}" => "Î", "\\\\^{I}" => "Î",
    "\\^{O}" => "Ô", "\\\\^{O}" => "Ô",
    "\\^{U}" => "Û", "\\\\^{U}" => "Û",

    // Umlaut (single, double, and triple backslash versions)
    "\\\"{a}" => "ä", "\\\\\"{a}" => "ä", "\\\\\\\"{a}" => "ä",
    "\\\"{e}" => "ë", "\\\\\"{e}" => "ë", "\\\\\\\"{e}" => "ë",
    "\\\"{i}" => "ï", "\\\\\"{i}" => "ï", "\\\\\\\"{i}" => "ï",
    "\\\"{o}" => "ö", "\\\\\"{o}" => "ö", "\\\\\\\"{o}" => "ö",
    "\\\"{u}" => "ü", "\\\\\"{u}" => "ü", "\\\\\\\"{u}" => "ü",
    "\\\"{A}" => "Ä", "\\\\\"{A}" => "Ä", "\\\\\\\"{A}" => "Ä",
    "\\\"{E}" => "Ë", "\\\\\"{E}" => "Ë", "\\\\\\\"{E}" => "Ë",
    "\\\"{I}" => "Ï", "\\\\\"{I}" => "Ï", "\\\\\\\"{I}" => "Ï",
    "\\\"{O}" => "Ö", "\\\\\"{O}" => "Ö", "\\\\\\\"{O}" => "Ö",
    "\\\"{U}" => "Ü", "\\\\\"{U}" => "Ü", "\\\\\\\"{U}" => "Ü",
    "\\\"{y}" => "ÿ", "\\\\\"{y}" => "ÿ", "\\\\\\\"{y}" => "ÿ",
    "\\\"{Y}" => "Ÿ", "\\\\\"{Y}" => "Ÿ", "\\\\\\\"{Y}" => "Ÿ",

    // Tilde (both single and double backslash versions)
    "\\~{a}" => "ã", "\\\\~{a}" => "ã",
    "\\~{n}" => "ñ", "\\\\~{n}" => "ñ",
    "\\~{o}" => "õ", "\\\\~{o}" => "õ",
    "\\~{A}" => "Ã", "\\\\~{A}" => "Ã",
    "\\~{N}" => "Ñ", "\\\\~{N}" => "Ñ",
    "\\~{O}" => "Õ", "\\\\~{O}" => "Õ",

    // Cedilla with braces (both single and double backslash versions)
    "\\c{c}" => "ç", "\\\\c{c}" => "ç",
    "\\c{C}" => "Ç", "\\\\c{C}" => "Ç",

    // Ring with braces (both single and double backslash versions)
    "\\r{a}" => "å", "\\\\r{a}" => "å",
    "\\r{A}" => "Å", "\\\\r{A}" => "Å",
};

/// Special LaTeX symbols and commands
static LATEX_SYMBOLS: phf::Map<&'static str, &'static str> = phf_map! {
    // Special ligatures and characters (both single and double backslash versions)
    "\\ae" => "æ", "\\AE" => "Æ", "\\\\ae" => "æ", "\\\\AE" => "Æ",
    "\\oe" => "œ", "\\OE" => "Œ", "\\\\oe" => "œ", "\\\\OE" => "Œ",
    "\\ss" => "ß", "\\\\ss" => "ß",
    "\\o " => "ø", "\\O " => "Ø", "\\\\o " => "ø", "\\\\O " => "Ø",  // With space absorption
    "\\o" => "ø", "\\O" => "Ø", "\\\\o" => "ø", "\\\\O" => "Ø",      // Without space
    "\\aa" => "å", "\\AA" => "Å", "\\\\aa" => "å", "\\\\AA" => "Å",

    // Greek letters (both single and double backslash versions)
    "\\alpha" => "α", "\\\\alpha" => "α",
    "\\beta" => "β", "\\\\beta" => "β",
    "\\gamma" => "γ", "\\\\gamma" => "γ",
    "\\delta" => "δ", "\\\\delta" => "δ",
    "\\epsilon" => "ε", "\\\\epsilon" => "ε",
    "\\varepsilon" => "ε", "\\\\varepsilon" => "ε",
    "\\zeta" => "ζ", "\\\\zeta" => "ζ",
    "\\eta" => "η", "\\\\eta" => "η",
    "\\theta" => "θ", "\\\\theta" => "θ",
    "\\vartheta" => "θ", "\\\\vartheta" => "θ",
    "\\iota" => "ι", "\\\\iota" => "ι",
    "\\kappa" => "κ", "\\\\kappa" => "κ",
    "\\lambda" => "λ", "\\\\lambda" => "λ",
    "\\mu" => "μ", "\\\\mu" => "μ",
    "\\nu" => "ν", "\\\\nu" => "ν",
    "\\xi" => "ξ", "\\\\xi" => "ξ",
    "\\pi" => "π", "\\\\pi" => "π",
    "\\varpi" => "π", "\\\\varpi" => "π",
    "\\rho" => "ρ", "\\\\rho" => "ρ",
    "\\varrho" => "ρ", "\\\\varrho" => "ρ",
    "\\sigma" => "σ", "\\\\sigma" => "σ",
    "\\varsigma" => "ς", "\\\\varsigma" => "ς",
    "\\tau" => "τ", "\\\\tau" => "τ",
    "\\upsilon" => "υ", "\\\\upsilon" => "υ",
    "\\phi" => "φ", "\\\\phi" => "φ",
    "\\varphi" => "φ", "\\\\varphi" => "φ",
    "\\chi" => "χ", "\\\\chi" => "χ",
    "\\psi" => "ψ", "\\\\psi" => "ψ",
    "\\omega" => "ω", "\\\\omega" => "ω",

    // Capital Greek letters (both single and double backslash versions)
    "\\Gamma" => "Γ", "\\\\Gamma" => "Γ",
    "\\Delta" => "Δ", "\\\\Delta" => "Δ",
    "\\Theta" => "Θ", "\\\\Theta" => "Θ",
    "\\Lambda" => "Λ", "\\\\Lambda" => "Λ",
    "\\Xi" => "Ξ", "\\\\Xi" => "Ξ",
    "\\Pi" => "Π", "\\\\Pi" => "Π",
    "\\Sigma" => "Σ", "\\\\Sigma" => "Σ",
    "\\Upsilon" => "Υ", "\\\\Upsilon" => "Υ",
    "\\Phi" => "Φ", "\\\\Phi" => "Φ",
    "\\Psi" => "Ψ", "\\\\Psi" => "Ψ",
    "\\Omega" => "Ω", "\\\\Omega" => "Ω",

    // Mathematical symbols (both single and double backslash versions)
    "\\infty" => "∞", "\\\\infty" => "∞",
    "\\partial" => "∂", "\\\\partial" => "∂",
    "\\nabla" => "∇", "\\\\nabla" => "∇",
    "\\pm" => "±", "\\\\pm" => "±",
    "\\mp" => "∓", "\\\\mp" => "∓",
    "\\sim" => "∼", "\\\\sim" => "∼",
    "\\times" => "×", "\\\\times" => "×",
    "\\div" => "÷", "\\\\div" => "÷",
    "\\leq" => "≤", "\\\\leq" => "≤",
    "\\geq" => "≥", "\\\\geq" => "≥",
    "\\neq" => "≠", "\\\\neq" => "≠",
    "\\approx" => "≈", "\\\\approx" => "≈",
    "\\equiv" => "≡", "\\\\equiv" => "≡",
    "\\subset" => "⊂", "\\\\subset" => "⊂",
    "\\supset" => "⊃", "\\\\supset" => "⊃",
    "\\subseteq" => "⊆", "\\\\subseteq" => "⊆",
    "\\supseteq" => "⊇", "\\\\supseteq" => "⊇",
    "\\in" => "∈", "\\\\in" => "∈",
    "\\notin" => "∉", "\\\\notin" => "∉",
    "\\cup" => "∪", "\\\\cup" => "∪",
    "\\cap" => "∩", "\\\\cap" => "∩",
    "\\rightarrow" => "→", "\\\\rightarrow" => "→",
    "\\leftarrow" => "←", "\\\\leftarrow" => "←",
    "\\leftrightarrow" => "↔", "\\\\leftrightarrow" => "↔",
    "\\Rightarrow" => "⇒", "\\\\Rightarrow" => "⇒",
    "\\Leftarrow" => "⇐", "\\\\Leftarrow" => "⇐",
    "\\Leftrightarrow" => "⇔", "\\\\Leftrightarrow" => "⇔",

    // Physics and advanced math symbols
    "\\hbar" => "ℏ", "\\\\hbar" => "ℏ",
    "\\hat{H}" => "Ĥ", "\\\\hat{H}" => "Ĥ",

    // Special mathematical expressions (specific patterns)
    "\\frac{\\partial}{\\partial t}" => "∂/∂t ",
    "\\\\frac{\\\\partial}{\\\\partial t}" => "∂/∂t ",

    // Punctuation and symbols (both single and double backslash versions)
    "\\ldots" => "…", "\\\\ldots" => "…",
    "\\dots" => "…", "\\\\dots" => "…",
    "\\cdots" => "⋯", "\\\\cdots" => "⋯",
    "\\&" => "&", "\\\\&" => "&",
    "\\%" => "%", "\\\\%" => "%",
    "\\$" => "$", "\\\\$" => "$",
    "\\#" => "#", "\\\\#" => "#",
    "\\{" => "{", "\\\\{" => "{",
    "\\}" => "}", "\\\\}" => "}",
    "\\textbackslash" => "\\", "\\\\textbackslash" => "\\",
    "\\_" => "_", "\\\\_" => "_",

    // Special case for four backslashes representing escaped backslash
    "\\\\\\\\" => "\\\\",

    // Quotes (both single and double backslash versions)
    "\\lq " => "'", "\\\\lq " => "'",  // Opening quotes with space absorption
    "\\lq" => "'", "\\\\lq" => "'",    // Opening quotes without space
    "\\rq" => "'", "\\\\rq" => "'",    // Closing quotes (no space absorption)
    "\\lqq " => "\u{201c}", "\\\\lqq " => "\u{201c}",  // Opening quotes with space absorption
    "\\lqq" => "\u{201c}", "\\\\lqq" => "\u{201c}",    // Opening quotes without space
    "\\rqq" => "\u{201d}", "\\\\rqq" => "\u{201d}",    // Closing quotes (no space absorption)

    // Spacing commands (both single and double backslash versions)
    "\\," => " ", "\\\\," => " ",
    // Note: removed "\\ " pattern as it interferes with literal backslashes

    // Degree symbol (both single and double backslash versions)
    "\\degree" => "°", "\\\\degree" => "°",
    "\\textdegree" => "°", "\\\\textdegree" => "°",

    // Copyright and related (both single and double backslash versions)
    "\\copyright" => "©", "\\\\copyright" => "©",
    "\\textcopyright" => "©", "\\\\textcopyright" => "©",
    "\\textregistered" => "®", "\\\\textregistered" => "®",
    "\\texttrademark" => "™", "\\\\texttrademark" => "™",

    // Currency (both single and double backslash versions)
    "\\pounds" => "£", "\\\\pounds" => "£",
    "\\textsterling" => "£", "\\\\textsterling" => "£",
};

/// Convert LaTeX escape sequences to Unicode
///
/// This function performs a single pass through the string, replacing
/// known LaTeX sequences with their Unicode equivalents.
///
/// # Performance
///
/// Uses a fast path for strings without LaTeX sequences to avoid
/// unnecessary scanning and allocation.
#[must_use]
pub fn latex_to_unicode(input: &str) -> String {
    // Fast path: if no backslashes or tildes, no LaTeX to convert
    if !input.contains('\\') && !input.contains('~') {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let mut chars = input.char_indices();

    while let Some((pos, ch)) = chars.next() {
        if ch == '\\' {
            // Look for the longest matching pattern starting at this position
            let remaining = &input[pos..];

            // Try to find the longest match
            let mut best_match: Option<(&str, &str)> = None;

            // Check all patterns, keeping the longest match
            // First check LATEX_BRACED (usually longest)
            for (pattern, replacement) in LATEX_BRACED.entries() {
                if remaining.starts_with(pattern)
                    && (best_match.is_none() || pattern.len() > best_match.unwrap().0.len())
                {
                    best_match = Some((pattern, replacement));
                }
            }

            // Then check LATEX_ACCENTS
            for (pattern, replacement) in LATEX_ACCENTS.entries() {
                if remaining.starts_with(pattern)
                    && (best_match.is_none() || pattern.len() > best_match.unwrap().0.len())
                {
                    best_match = Some((pattern, replacement));
                }
            }

            // Then check LATEX_SYMBOLS
            for (pattern, replacement) in LATEX_SYMBOLS.entries() {
                if remaining.starts_with(pattern)
                    && (best_match.is_none() || pattern.len() > best_match.unwrap().0.len())
                {
                    best_match = Some((pattern, replacement));
                }
            }

            if let Some((pattern, replacement)) = best_match {
                result.push_str(replacement);

                // Skip the matched characters
                for _ in 1..pattern.len() {
                    chars.next();
                }
            } else {
                // No pattern matched, keep the backslash
                result.push(ch);
            }
        } else if ch == '~' {
            // Check if this is a standalone tilde (not part of \~)
            if pos == 0 || !input[..pos].ends_with('\\') {
                // Non-breaking space
                result.push(' ');
            } else {
                // Part of \~ sequence, keep it
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_accents() {
        assert_eq!(latex_to_unicode("\\'e"), "é");
        assert_eq!(latex_to_unicode("\\'{e}"), "é");
        assert_eq!(latex_to_unicode("\\\"o"), "ö");
        assert_eq!(latex_to_unicode("\\\"{o}"), "ö");
        assert_eq!(latex_to_unicode("\\~n"), "ñ");
        assert_eq!(latex_to_unicode("\\^a"), "â");
        assert_eq!(latex_to_unicode("\\`u"), "ù");
    }

    #[test]
    fn test_cedilla_and_ring() {
        assert_eq!(latex_to_unicode("\\c{c}"), "ç");
        assert_eq!(latex_to_unicode("\\c C"), "Ç");
        assert_eq!(latex_to_unicode("\\r{a}"), "å");
        assert_eq!(latex_to_unicode("\\r A"), "Å");
        assert_eq!(latex_to_unicode("\\aa"), "å");
        assert_eq!(latex_to_unicode("\\AA"), "Å");
    }

    #[test]
    fn test_ligatures() {
        assert_eq!(latex_to_unicode("\\ae"), "æ");
        assert_eq!(latex_to_unicode("\\AE"), "Æ");
        assert_eq!(latex_to_unicode("\\oe"), "œ");
        assert_eq!(latex_to_unicode("\\ss"), "ß");
        assert_eq!(latex_to_unicode("\\o"), "ø");
        assert_eq!(latex_to_unicode("\\O"), "Ø");
    }

    #[test]
    fn test_mixed_text() {
        assert_eq!(latex_to_unicode("Fran\\c{c}ois R\\'emi"), "François Rémi");
        assert_eq!(
            latex_to_unicode("M\\\"uller and Schr\\\"{o}dinger"),
            "Müller and Schrödinger"
        );
        assert_eq!(latex_to_unicode("Jos\\'e Garc\\'ia"), "José García");
    }

    #[test]
    fn test_no_latex() {
        let plain = "This has no LaTeX";
        assert_eq!(latex_to_unicode(plain), plain);
    }

    #[test]
    fn test_greek_letters() {
        assert_eq!(latex_to_unicode("\\alpha-\\beta"), "α-β");
        assert_eq!(latex_to_unicode("\\gamma \\delta"), "γ δ");
        assert_eq!(latex_to_unicode("\\Gamma\\Delta"), "ΓΔ");
    }

    #[test]
    fn test_symbols() {
        assert_eq!(latex_to_unicode("\\ldots"), "…");
        assert_eq!(latex_to_unicode("\\&"), "&");
        assert_eq!(latex_to_unicode("\\%"), "%");
        assert_eq!(latex_to_unicode("\\copyright"), "©");
    }

    #[test]
    fn test_mathematical_symbols() {
        assert_eq!(latex_to_unicode("\\leq"), "≤");
        assert_eq!(latex_to_unicode("\\geq"), "≥");
        assert_eq!(latex_to_unicode("\\neq"), "≠");
        assert_eq!(latex_to_unicode("\\pm"), "±");
        assert_eq!(latex_to_unicode("\\times"), "×");
    }

    #[test]
    fn test_tildes() {
        // Standalone tildes become spaces
        assert_eq!(latex_to_unicode("word~word"), "word word");
        // LaTeX tildes are accents
        assert_eq!(latex_to_unicode("\\~n"), "ñ");
        // Mixed
        assert_eq!(latex_to_unicode("Se\\~nor~Garc\\'ia"), "Señor García");
    }

    #[test]
    fn test_performance_fast_path() {
        let plain = "This is plain ASCII text with no LaTeX sequences whatsoever";
        // Should use fast path and return identical string
        assert_eq!(latex_to_unicode(plain), plain);
    }

    #[test]
    fn test_complex_scientific_text() {
        let input = "The \\alpha-particle decay rate follows \\lambda \\propto e^{-\\gamma t}";
        // Note: Complex math expressions in braces are not fully supported
        // This test shows current behavior - only simple substitutions
        assert!(latex_to_unicode(input).contains("α"));
        assert!(latex_to_unicode(input).contains("λ"));
    }

    #[test]
    fn test_edge_cases() {
        // Incomplete sequences should be left alone
        assert_eq!(latex_to_unicode("\\"), "\\");
        assert_eq!(latex_to_unicode("\\'"), "\\'");
        assert_eq!(latex_to_unicode("\\'{"), "\\'{");

        // Unknown sequences should be left alone
        assert_eq!(latex_to_unicode("\\xyz"), "\\xyz");
        assert_eq!(latex_to_unicode("\\unknown{test}"), "\\unknown{test}");

        // Test specific failing case
        assert_eq!(
            latex_to_unicode("\\alpha and \\beta particles"),
            "α and β particles"
        );
    }
}
