//! The call of the inbox: `mark_read`, which marks the notifications of one issue read in Linear.

use std::sync::Arc;

use pencere::Options;
use serde::{Deserialize, Serialize};

use super::issue::check_id;
use super::{Failure, call};
use crate::linear::Error;
use crate::session::Session;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarkRead {
    id: String,
}

#[derive(Serialize)]
struct Done {}

fn mark_read(session: &Session, params: MarkRead) -> Result<Done, Failure> {
    check_id(&params.id)?;
    if session.state().key.is_none() {
        return Err(Failure::NoKey(
            "there is no key to ask Linear with".to_owned(),
        ));
    }
    session.mark_read(&params.id).map_err(|error| match error {
        Error::Refused | Error::Unreachable(_) => Failure::Linear(error),
        other => Failure::ChangeFailed(other.to_string()),
    })?;
    Ok(Done {})
}

pub fn calls(options: Options, session: &Arc<Session>) -> Options {
    call(options, session, "mark_read", mark_read)
}
