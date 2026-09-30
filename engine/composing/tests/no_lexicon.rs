//! Integration tests that need the process-wide lexicon never installed:
//! `FetchAtPos` answers an empty carrier and the nailed-join compound
//! oracle stays off (`台` + `語` commit `tâi gí`, not `tâi-gí`).

mod common;

mod continuous_commit_resolution;
mod dispatch_continuous;
mod lifecycle;
