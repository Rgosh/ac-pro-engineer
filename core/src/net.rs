//! Where a blocking HTTP request is allowed to run.
//!
//! `reqwest::blocking` builds a private tokio runtime and drops it while
//! constructing a client. Dropping a runtime from a thread that is already
//! inside one panics — *"Cannot drop a runtime in a context where blocking is
//! not allowed"* — and takes the application with it.
//!
//! That is not a theoretical hazard. Pressing `[B]` on the launcher's overlay
//! card called [`crate::overlay::bridge_update::latest_published`] straight
//! from the terminal's key handler, which ran inside `#[tokio::main]`, and the
//! application died before drawing another frame. Every other blocking request
//! in this crate happened to be made from a `thread::spawn`, which is why only
//! this one key crashed.
//!
//! A plain OS thread carries no runtime context, so running the request on one
//! is correct wherever the caller happens to be. One thread spawn against a
//! network round trip is not a cost worth reasoning about, and it means a
//! future caller cannot reintroduce the crash by calling from the wrong place.
//!
//! **Every blocking request in this crate goes through here**, except the
//! updater's two — those are the first thing inside a `thread::spawn` of their
//! own, and are long closures reporting progress and writing files rather than
//! one request that returns a value. A new one belongs here unless it is
//! demonstrably already on a fresh thread.

use std::thread;

/// Run `work` on a thread that is not inside any async runtime, and hand back
/// what it returned.
///
/// Scoped, so `work` may borrow — the caller blocks until it finishes either
/// way. A panic inside `work` is resumed on this thread rather than swallowed,
/// so the failure looks exactly as it would have without the hop.
pub fn off_runtime<T, F>(work: F) -> T
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    match thread::scope(|scope| scope.spawn(work).join()) {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_hands_back_what_the_work_returned() {
        assert_eq!(off_runtime(|| 2 + 2), 4);
    }

    /// It has to be able to borrow, or every caller has to clone its arguments
    /// to make a request with them.
    #[test]
    fn the_work_can_borrow_from_the_caller() {
        let url = String::from("https://example.invalid/releases");
        let length = off_runtime(|| url.len());
        assert_eq!(length, url.len());
    }

    /// The crash this exists to prevent, reproduced without a network: simply
    /// *building* a blocking client is what drops the private runtime, so this
    /// test panics on the direct call and passes through the hop.
    #[test]
    fn a_blocking_client_can_be_built_from_inside_an_async_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build a runtime");

        let built = runtime.block_on(async {
            off_runtime(|| reqwest::blocking::Client::builder().build().is_ok())
        });

        assert!(
            built,
            "the client built, and dropping its runtime did not panic"
        );
    }
}
