//! The call `filter_options`: what the filters of a list offer to choose from (the status names, the
//! people and the labels). The projects are not here: the page has them among the lists it can go to.
//! What the filters are set to goes by `view`.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::{Failure, call};
use crate::session::Session;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nothing {}

#[derive(Debug, Serialize)]
struct StateMessage {
    name: String,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct PersonMessage {
    id: String,
    name: String,
    /// The person themself: the menu says "Me" for them, and the filter by `me`.
    me: bool,
}

#[derive(Debug, Serialize)]
struct LabelMessage {
    name: String,
    color: String,
}

#[derive(Debug, Serialize)]
struct OptionsMessage {
    states: Vec<StateMessage>,
    people: Vec<PersonMessage>,
    labels: Vec<LabelMessage>,
}

fn filter_options(session: &Session, _: Nothing) -> Result<OptionsMessage, Failure> {
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    let options = session.filter_options().map_err(Failure::Linear)?;
    Ok(OptionsMessage {
        states: options
            .states
            .into_iter()
            .map(|state| StateMessage {
                name: state.name,
                kind: state.kind.name(),
            })
            .collect(),
        people: options
            .people
            .into_iter()
            .map(|person| PersonMessage {
                id: person.id,
                name: person.name,
                me: person.me,
            })
            .collect(),
        labels: options
            .labels
            .into_iter()
            .map(|label| LabelMessage {
                name: label.name,
                color: label.color,
            })
            .collect(),
    })
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "filter_options", filter_options)
}
