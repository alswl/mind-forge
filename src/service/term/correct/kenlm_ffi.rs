//! Safe Rust wrapper over the vendored KenLM C ABI query shim (spec 055).
//!
//! The shim is compiled from `vendor/kenlm/mf_kenlm_shim.{h,cc}` via `build.rs`.
//! This module exposes a minimal, owned, panic-safe interface for loading models,
//! scoring sentences, computing perplexity, and checking vocabulary coverage.

use std::os::raw::c_char;
use std::path::Path;

/// Opaque C model handle.
#[repr(C)]
struct MfKenlmModel {
    _private: [u8; 0],
}

extern "C" {
    fn mf_kenlm_load(path: *const c_char) -> *mut MfKenlmModel;
    fn mf_kenlm_free(model: *mut MfKenlmModel);
    fn mf_kenlm_order(model: *const MfKenlmModel) -> u8;
    fn mf_kenlm_score(model: *const MfKenlmModel, sentence: *const c_char) -> f64;
    fn mf_kenlm_perplexity(model: *const MfKenlmModel, sentence: *const c_char) -> f64;
    fn mf_kenlm_contains_all(model: *const MfKenlmModel, sentence: *const c_char) -> i32;
    fn mf_kenlm_version() -> *const c_char;
}

/// Owned KenLM model handle.
///
/// Load a binary or ARPA model with [`KenLmModel::load`]. Scores and perplexity
/// are computed over whitespace-tokenized sentences. The model is automatically
/// freed on drop.
pub struct KenLmModel {
    inner: *mut MfKenlmModel,
}

// SAFETY: The C shim is single-threaded but the underlying KenLM model is
// read-only after loading. All query functions are const on the model handle.
unsafe impl Send for KenLmModel {}
unsafe impl Sync for KenLmModel {}

impl KenLmModel {
    /// Load a binary or ARPA KenLM model from `path`.
    ///
    /// Returns `None` if the model cannot be loaded.
    pub fn load(path: &Path) -> Option<Self> {
        let c_path = path.to_string_lossy();
        let c_str = std::ffi::CString::new(c_path.as_ref()).ok()?;
        let inner = unsafe { mf_kenlm_load(c_str.as_ptr()) };
        if inner.is_null() {
            None
        } else {
            Some(Self { inner })
        }
    }

    /// Return the n-gram order (e.g. 5).
    pub fn order(&self) -> u8 {
        unsafe { mf_kenlm_order(self.inner) }
    }

    /// Total log10 probability of a whitespace-tokenized sentence.
    ///
    /// Includes end-of-sentence token. Returns `f64::NAN` on error or empty input.
    pub fn score(&self, sentence: &str) -> f64 {
        let c_str = match std::ffi::CString::new(sentence) {
            Ok(s) => s,
            Err(_) => return f64::NAN,
        };
        unsafe { mf_kenlm_score(self.inner, c_str.as_ptr()) }
    }

    /// Perplexity (including end-of-sentence) of a whitespace-tokenized sentence.
    ///
    /// Returns `f64::NAN` on error or empty input.
    pub fn perplexity(&self, sentence: &str) -> f64 {
        let c_str = match std::ffi::CString::new(sentence) {
            Ok(s) => s,
            Err(_) => return f64::NAN,
        };
        unsafe { mf_kenlm_perplexity(self.inner, c_str.as_ptr()) }
    }

    /// Returns `true` when every token in the sentence is in-vocabulary.
    pub fn contains_all(&self, sentence: &str) -> bool {
        let c_str = match std::ffi::CString::new(sentence) {
            Ok(s) => s,
            Err(_) => return false,
        };
        unsafe { mf_kenlm_contains_all(self.inner, c_str.as_ptr()) != 0 }
    }

    /// Return the static shim version string, e.g. `"kenlm-4cb443e"`.
    pub fn version() -> &'static str {
        let ptr = unsafe { mf_kenlm_version() };
        if ptr.is_null() {
            return "";
        }
        unsafe {
            std::ffi::CStr::from_ptr(ptr)
                .to_str()
                .unwrap_or("")
        }
    }
}

impl Drop for KenLmModel {
    fn drop(&mut self) {
        if !self.inner.is_null() {
            unsafe { mf_kenlm_free(self.inner) };
            self.inner = std::ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tiny_arpa_path() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join("tests/fixtures/asr/tiny_model.arpa")
    }

    #[test]
    fn load_valid_arpa() {
        let model = KenLmModel::load(&tiny_arpa_path());
        assert!(model.is_some(), "must load tiny ARPA fixture");
    }

    #[test]
    fn load_nonexistent_file() {
        let model = KenLmModel::load(Path::new("/nonexistent/path/model.klm"));
        assert!(model.is_none());
    }

    #[test]
    fn order_is_positive() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        assert!(model.order() > 0, "order must be positive: {}", model.order());
    }

    #[test]
    fn score_gives_finite_value() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        let s = model.score("在线 服务 上线 了");
        assert!(s.is_finite(), "score must be finite: {s}");
    }

    #[test]
    fn perplexity_gives_finite_value() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        let ppl = model.perplexity("在线 服务 上线 了");
        assert!(ppl.is_finite(), "perplexity must be finite: {ppl}");
        assert!(ppl > 0.0);
    }

    #[test]
    fn perplexity_empty_input_is_nan() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        assert!(model.perplexity("").is_nan());
    }

    #[test]
    fn score_empty_input_is_nan() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        assert!(model.score("").is_nan());
    }

    #[test]
    fn contains_all_known_tokens() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        assert!(model.contains_all("在 线 服务"));
        assert!(model.contains_all("在线 服务 上线 了"));
    }

    #[test]
    fn contains_all_rejects_oov() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        // "ZYXWVUT" is not in the tiny ARPA vocabulary.
        assert!(!model.contains_all("ZYXWVUT"));
    }

    #[test]
    fn deterministic_repeated_queries() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        let a = model.perplexity("在线 服务 上线 了");
        let b = model.perplexity("在线 服务 上线 了");
        assert!(
            (a - b).abs() < 1e-12,
            "repeated queries must be deterministic: a={a}, b={b}"
        );
    }

    #[test]
    fn correct_sentence_lower_perplexity_than_error() {
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        // 在线服务上线了 (correct "服务") vs 在线服物上线了 (error "服物")
        let ppl_correct = model.perplexity("在线 服务 上线 了");
        let ppl_error = model.perplexity("在线 服物 上线 了");
        assert!(
            ppl_correct < ppl_error,
            "correct sentence should have lower PPL: {ppl_correct} vs {ppl_error}"
        );
    }

    #[test]
    fn version_is_nonempty() {
        let v = KenLmModel::version();
        assert!(!v.is_empty(), "version must be non-empty");
        assert!(v.starts_with("kenlm-"), "version must start with 'kenlm-': {v}");
    }

    #[test]
    fn model_drop_doesnt_double_free() {
        // Dropping a loaded model should not panic.
        let model = KenLmModel::load(&tiny_arpa_path()).unwrap();
        drop(model);
    }
}
