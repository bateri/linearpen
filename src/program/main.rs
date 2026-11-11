//! linearpen: your Linear issues in the terminal pane, and an agent started on the ones you pick.
//! In a terminal that shows pencere views the client opens in the pane; in one that does not, the
//! program prints the issues as text and ends.
//!
//! It reads, and writes one thing: the issue the person makes in the form of the new issue. Linear
//! is spoken to with `curl` and nothing else (`linear/`).
//!
//! The program asks Linear and keeps what it was told; the page (`src/view/`, in pencere's kit) is
//! the client, drawn by the terminal, and asks the program for what it needs with the calls and the
//! topics of `docs/calls.md`.
//!
//! This file is the entry, and the one place that reads the environment: it reads the command line,
//! says where the key and the state are, and prints the text where there is no view.
//!
//!   linear/   what is asked of Linear and how the answers are read; knows nothing of the page
//!   model/    the list: filtered, grouped and sorted; knows nothing of the page
//!   state/    the key and the files kept between runs; knows nothing of the page
//!   text/     the issues as text for a terminal that shows no views; knows nothing of the page
//!   work/     handing issues to an agent in a pane of bateri; knows nothing of the page

mod api;
mod command_line;
mod icons;
mod linear;
mod model;
#[cfg(test)]
mod page_shots;
#[cfg(test)]
mod page_tests;
mod session;
mod state;
mod text;
mod work;

use std::process::ExitCode;
use std::sync::{Arc, Mutex, PoisonError};

use pencere::{EndedBy, NoViewReason, Options, Outcome};

use command_line::Asked;
use linear::transport::{Curl, DEFAULT_URL};
use linear::{Error, Scope, Transport};
use session::{REFRESH_EVERY, Session};
use state::Paths;
use state::credentials;
use text::summary::Summary;
use work::Setup;
use work::runner::System;

/// The words under the text of a terminal that shows no views.
const NO_VIEWS: &str = "This terminal shows no views. linearpen opens as a Linear client in a terminal that\nhosts pencere (bateri); run it there, or over ssh from one.";

/// The environment, which is read here and nowhere else.
fn lookup(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Why there is no text: what to say on stderr, and the status to end with.
struct Failed {
    words: String,
    status: u8,
}

/// The issues as text: both lists, from Linear, with `note` under them.
fn summary(note: Option<&str>) -> Result<String, Failed> {
    let paths = Paths::resolve(&lookup);
    let found = credentials::find(lookup("LINEAR_API_KEY").as_deref(), paths.config.as_deref())
        .map_err(|problem| Failed {
            words: problem.words(),
            status: 2,
        })?;
    if let (true, credentials::Source::File(file)) = (found.loose, &found.source) {
        eprintln!(
            "linearpen: {} can be read by other users; chmod 600 it",
            file.display()
        );
    }
    let transport =
        Curl::new(lookup("LINEARPEN_API_URL").unwrap_or_else(|| DEFAULT_URL.to_owned()));
    let failed = |error: Error| Failed {
        words: match error {
            Error::Refused => format!(
                "linearpen: Linear did not accept the key from {}",
                found.source.words()
            ),
            other => format!("linearpen: {other}"),
        },
        status: 1,
    };
    let mine = linear::fetch(&transport, &found.key, Scope::Mine).map_err(failed)?;
    let all = linear::fetch(&transport, &found.key, Scope::All).map_err(failed)?;
    Ok(Summary {
        viewer: &mine.viewer,
        mine: &mine.listing,
        all: &all.listing,
    }
    .text(note))
}

/// Prints the text, and says how the program ends.
fn print_summary(note: Option<&str>) -> u8 {
    match summary(note) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(failed) => {
            eprintln!("{}", failed.words);
            failed.status
        }
    }
}

/// What `serve` is given: the app, its name and icons, the calls and the topic of `api/`, and the
/// text for a terminal that shows no views, whose status goes to `fallen_back`.
fn options(session: &Arc<Session>, fallen_back: &Arc<Mutex<Option<u8>>>) -> Options {
    let mut options = Options::new()
        .app(pencere::app!())
        .program("linearpen")
        // What a terminal writes above the view.
        .title("Linear");
    for icon in icons::all() {
        options = options.icon(icon);
    }
    let options = api::serve(options, session);
    let status = Arc::clone(fallen_back);
    options.fallback(move |why| {
        // A pipe is a script's: it is given the text alone.
        let note = (why.reason != NoViewReason::NoTty).then_some(NO_VIEWS);
        *status.lock().unwrap_or_else(PoisonError::into_inner) = Some(print_summary(note));
    })
}

/// The session of a run: Linear is asked with `curl`, at the address the environment gives (for the
/// tests) or Linear's own.
fn session() -> Arc<Session> {
    let transport: Arc<dyn Transport> = Arc::new(Curl::new(
        lookup("LINEARPEN_API_URL").unwrap_or_else(|| DEFAULT_URL.to_owned()),
    ));
    Arc::new(
        Session::new(
            transport,
            Paths::resolve(&lookup),
            lookup("LINEAR_API_KEY").as_deref(),
            REFRESH_EVERY,
        )
        .with_work(
            Setup::resolve(&lookup, std::env::current_dir().ok()),
            Arc::new(System),
        ),
    )
}

fn main() -> ExitCode {
    let line = match command_line::read(std::env::args().skip(1)) {
        Asked::Run(line) => line,
        Asked::Say(words) => {
            print!("{words}");
            return ExitCode::SUCCESS;
        }
        Asked::Refuse(words) => {
            eprintln!("{words}");
            return ExitCode::from(2);
        }
    };
    if line.text {
        return ExitCode::from(print_summary(None));
    }
    let fallen_back = Arc::new(Mutex::new(None));
    let session = session();
    let status = match pencere::serve(options(&session, &fallen_back)) {
        // The terminal took the client down: what it said is the one trace of why, so it is left
        // under the prompt rather than lost with the view.
        Ok(Outcome::Closed(closed)) if closed.by == EndedBy::Error => {
            let words = closed
                .message
                .as_deref()
                .unwrap_or("the terminal gave no reason");
            eprintln!("linearpen: the client stopped: {words}");
            1
        }
        Ok(outcome) => fallen_back
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .unwrap_or_else(|| outcome.shell_status()),
        Err(why) => {
            eprintln!("linearpen: {why}");
            1
        }
    };
    ExitCode::from(status)
}
