use miette::{Diagnostic, SourceSpan, NamedSource};
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
pub enum MyError {
    #[error("oops")]
    Op {
        #[source_code]
        src: NamedSource<String>,
        #[label("here")]
        span: Option<SourceSpan>,
    }
}
fn main() {}
