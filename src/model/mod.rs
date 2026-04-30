pub mod article;
#[allow(dead_code)]
pub mod asset;
// config 被 src/service/config.rs 引用，无需 dead_code 豁免
pub mod config;
pub mod index;
#[allow(dead_code)]
pub mod project;
#[allow(dead_code)]
pub mod source;
#[allow(dead_code)]
pub mod term;

pub mod manifest;
