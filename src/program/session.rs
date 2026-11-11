//! What the program holds for the page: the key, the two lists as Linear last said them, what the
//! page is looking at (the scope, the filter, the groups the person folded), and the worker that asks
//! Linear again, at the page's word and every few minutes.
//!
//! The worker is one thread. It asks for both lists (My issues and All issues), one after the other,
//! and puts them in `State` as a whole; a failure leaves the lists as they were and says why. Whoever
//! changes `State` bumps `generation` and wakes the topic's threads and the worker.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::linear::{self, Error, Issue, Key, Listing, Scope, Transport, Viewer};
use crate::state::Paths;
use crate::state::credentials::{self, Problem, Source};
use crate::work::runner::{Runner, System};
use crate::work::{Setup, bateri};

/// How long the lists are left before they are asked for again, while the page is open.
pub const REFRESH_EVERY: Duration = Duration::from_secs(300);

/// How many opened issues are kept before they are let go of.
const DETAILS_KEPT: usize = 64;

/// The agent an issue was handed to, and the pane it works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Working {
    pub agent: &'static str,
    pub pane: String,
}

/// What the program needs to hand issues to an agent: what the machine says, and what runs the
/// commands.
pub struct Work {
    pub setup: Setup,
    pub runner: Arc<dyn Runner>,
}

/// What the page is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub scope: Scope,
    pub filter: String,
    /// The groups the person folded (`true`) or opened (`false`), by key; one not here is as it
    /// begins (backlog and triage folded).
    pub closed: HashMap<String, bool>,
}

/// The two lists, and whose they are.
#[derive(Debug, Clone, PartialEq)]
pub struct Lists {
    pub viewer: Viewer,
    pub mine: Listing,
    pub all: Listing,
}

#[derive(Debug)]
pub struct State {
    pub key: Option<Key>,
    pub source: Option<Source>,
    /// Why the key there is cannot be used (a key that is not one, a file that cannot be read).
    pub problem: Option<String>,
    pub lists: Option<Lists>,
    /// The worker is asking Linear now.
    pub refreshing: bool,
    /// What went wrong the last time Linear was asked.
    pub error: Option<Error>,
    /// When the lists were told, seconds since the epoch.
    pub updated: Option<u64>,
    pub view: View,
    /// The number of the last `view` call that set each member of `view` (scope, filter, closed),
    /// which are made at the same time and may be told out of their order.
    view_seen: [u64; 3],
    /// The issues an agent was started on whose pane has not ended, by Linear's id.
    pub working: HashMap<String, Working>,
    /// Issues opened, as Linear told them (with their description and comments), by id: they are
    /// kept for the run, so that an issue seen again is not asked for again.
    details: HashMap<String, Issue>,
    generation: u64,
    refresh_wanted: bool,
    worker: bool,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    transport: Arc<dyn Transport>,
    every: Duration,
}

pub struct Session {
    shared: Arc<Shared>,
    pub paths: Paths,
    pub work: Work,
}

/// Now, seconds since the epoch.
pub fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells everyone waiting that `State` changed.
    fn changed(&self, state: &mut State) {
        state.generation += 1;
        self.wake.notify_all();
    }
}

/// Asks Linear for both lists.
fn fetch_both(transport: &dyn Transport, key: &Key) -> Result<Lists, Error> {
    let mine = linear::fetch(transport, key, Scope::Mine)?;
    let all = linear::fetch(transport, key, Scope::All)?;
    Ok(Lists {
        viewer: mine.viewer,
        mine: mine.listing,
        all: all.listing,
    })
}

/// The worker: asks when it is asked to, and again when `every` has passed.
fn work(shared: &Shared) {
    let mut next = Instant::now() + shared.every;
    loop {
        let key = {
            let mut state = shared.lock();
            loop {
                if state.closed {
                    return;
                }
                let due = Instant::now() >= next;
                if (state.refresh_wanted || due) && state.key.is_some() {
                    break;
                }
                if due {
                    // Nothing to ask with: look again in a while.
                    next = Instant::now() + shared.every;
                }
                let wait = next.saturating_duration_since(Instant::now());
                state = shared
                    .wake
                    .wait_timeout(state, wait.max(Duration::from_millis(10)))
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
            state.refresh_wanted = false;
            state.refreshing = true;
            shared.changed(&mut state);
            state.key.clone()
        };
        let Some(key) = key else { continue };
        let got = fetch_both(shared.transport.as_ref(), &key);
        let mut state = shared.lock();
        state.refreshing = false;
        match got {
            Ok(lists) => {
                state.lists = Some(lists);
                state.error = None;
                state.updated = Some(now_seconds());
            }
            Err(error) => state.error = Some(error),
        }
        shared.changed(&mut state);
        next = Instant::now() + shared.every;
    }
}

impl Session {
    /// A session that reaches Linear by `transport`, with the key of `env_key` or the one in the
    /// file of `paths`, asking again every `every`.
    pub fn new(
        transport: Arc<dyn Transport>,
        paths: Paths,
        env_key: Option<&str>,
        every: Duration,
    ) -> Self {
        let (key, source, problem) = match credentials::find(env_key, paths.config.as_deref()) {
            Ok(found) => (Some(found.key), Some(found.source), None),
            Err(Problem::Missing { .. }) => (None, None, None),
            Err(other) => (None, None, Some(other.words())),
        };
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                key,
                source,
                problem,
                lists: None,
                refreshing: false,
                error: None,
                updated: None,
                view: View {
                    scope: Scope::Mine,
                    filter: String::new(),
                    closed: HashMap::new(),
                },
                working: HashMap::new(),
                details: HashMap::new(),
                view_seen: [0; 3],
                generation: 0,
                refresh_wanted: false,
                worker: false,
                closed: false,
            }),
            wake: Condvar::new(),
            transport,
            every,
        });
        // Nothing is started until the machine is said (`with_work`): no bateri, no agent.
        let work = Work {
            setup: Setup::default(),
            runner: Arc::new(System),
        };
        Self {
            shared,
            paths,
            work,
        }
    }

    /// The session with what the machine says and what runs the commands (`bateri`, `git`).
    #[must_use]
    pub fn with_work(mut self, setup: Setup, runner: Arc<dyn Runner>) -> Self {
        self.work = Work { setup, runner };
        self
    }

    /// The issue with Linear's `id` as the lists hold it, with no asking of Linear.
    pub fn listed(&self, id: &str) -> Option<Issue> {
        let state = self.shared.lock();
        let lists = state.lists.as_ref()?;
        lists
            .mine
            .issues
            .iter()
            .chain(&lists.all.issues)
            .find(|issue| issue.id == id)
            .cloned()
    }

    /// An agent was started on the issues `ids`, in `pane`: they carry its mark until the pane's
    /// program ends, which a thread waits for.
    pub fn watch(&self, ids: &[String], agent: &'static str, pane: &str) {
        {
            let mut state = self.shared.lock();
            for id in ids {
                state.working.insert(
                    id.clone(),
                    Working {
                        agent,
                        pane: pane.to_owned(),
                    },
                );
            }
            self.shared.changed(&mut state);
        }
        let shared = Arc::clone(&self.shared);
        let runner = Arc::clone(&self.work.runner);
        let pane = pane.to_owned();
        let waited = pane.clone();
        let started = thread::Builder::new()
            .name("linearpen-wait".to_owned())
            .spawn(move || {
                bateri::wait(runner.as_ref(), &waited);
                let mut state = shared.lock();
                state.working.retain(|_, working| working.pane != waited);
                shared.changed(&mut state);
            });
        if started.is_err() {
            // Nothing will say when it ends: no mark is better than one that stays.
            let mut state = self.shared.lock();
            state.working.retain(|_, working| working.pane != pane);
            self.shared.changed(&mut state);
        }
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        self.shared.lock()
    }

    /// Starts the worker, once, and has it ask now. Nothing is asked where there is no key.
    pub fn start(&self) {
        let mut state = self.shared.lock();
        if state.key.is_some() {
            state.refresh_wanted = true;
            if !state.worker {
                state.worker = true;
                let shared = Arc::clone(&self.shared);
                let started = thread::Builder::new()
                    .name("linearpen-worker".to_owned())
                    .spawn(move || work(&shared));
                if started.is_err() {
                    state.worker = false;
                    state.error = Some(Error::Failed("the program cannot ask Linear".to_owned()));
                }
            }
            self.shared.changed(&mut state);
        }
    }

    /// What a page that wants the lists does: starts the worker and has it ask, unless it is going
    /// already (the page boots and subscribes, and the lists are asked for once).
    pub fn begin(&self) {
        let going = self.shared.lock().worker;
        if !going {
            self.start();
        }
    }

    /// Asks Linear again, now.
    pub fn refresh(&self) {
        self.start();
    }

    /// Gives the session this key, as the one to use (the page connected one that Linear took).
    pub fn use_key(&self, key: Key, source: Source) {
        {
            let mut state = self.shared.lock();
            state.key = Some(key);
            state.source = Some(source);
            state.problem = None;
            state.error = None;
        }
        self.start();
    }

    /// The issue with Linear's `id` (or its identifier), with its description and comments: from
    /// what was kept, else asked of Linear (without holding the state while it is).
    pub fn detail(&self, id: &str) -> Result<Issue, Error> {
        if let Some(kept) = self.shared.lock().details.get(id) {
            return Ok(kept.clone());
        }
        self.detail_fresh(id)
    }

    /// The same, always asked of Linear (and kept): what is handed to an agent is what Linear says
    /// now, not what the panel showed when it was first opened.
    pub fn detail_fresh(&self, id: &str) -> Result<Issue, Error> {
        let key = self.shared.lock().key.clone();
        let Some(key) = key else {
            return Err(Error::Failed(
                "there is no key to ask Linear with".to_owned(),
            ));
        };
        let issue = linear::fetch_issue(self.shared.transport.as_ref(), &key, id)?;
        let mut state = self.shared.lock();
        if state.details.len() >= DETAILS_KEPT {
            state.details.clear();
        }
        state.details.insert(id.to_owned(), issue.clone());
        Ok(issue)
    }

    /// A page that begins again numbers its `view` calls from the start: what was held of the last
    /// one's numbers is let go.
    pub fn forget_view_order(&self) {
        self.shared.lock().view_seen = [0; 3];
    }

    /// Changes what the page is looking at, and wakes whoever shows it. The page numbers its calls
    /// (`seq`): a member that a call with a higher number has set is left as it is, for calls run at
    /// the same time and may come out of their order. A call with no number is the latest.
    pub fn set_view(
        &self,
        seq: Option<u64>,
        scope: Option<Scope>,
        filter: Option<String>,
        closed: Option<HashMap<String, bool>>,
    ) {
        let mut state = self.shared.lock();
        // Only a member that the call sets is held to the order: one it does not set is not its.
        let mut allowed = |member: usize, sets: bool| match seq {
            _ if !sets => false,
            Some(seq) if seq < state.view_seen[member] => false,
            Some(seq) => {
                state.view_seen[member] = seq;
                true
            }
            None => true,
        };
        let (scope_ok, filter_ok, closed_ok) = (
            allowed(0, scope.is_some()),
            allowed(1, filter.is_some()),
            allowed(2, closed.is_some()),
        );
        if let Some(scope) = scope.filter(|_| scope_ok) {
            state.view.scope = scope;
        }
        if let Some(filter) = filter.filter(|_| filter_ok) {
            state.view.filter = filter;
        }
        if let Some(closed) = closed.filter(|_| closed_ok) {
            state.view.closed = closed;
        }
        self.shared.changed(&mut state);
    }

    /// Waits until `State` is not at `seen` any more, or `limit` has passed: gives its generation.
    pub fn wait_change(&self, seen: u64, limit: Duration) -> u64 {
        let state = self.shared.lock();
        let (state, _) = self
            .shared
            .wake
            .wait_timeout_while(state, limit, |state| {
                state.generation == seen && !state.closed
            })
            .unwrap_or_else(PoisonError::into_inner);
        state.generation
    }

    /// Where `State` is now, for `wait_change`.
    pub fn generation(&self) -> u64 {
        self.shared.lock().generation
    }

    /// Wakes the ones that wait on this session (a subscription ended).
    pub fn wake(&self) {
        self.shared.wake.notify_all();
    }

    /// The transport the session asks Linear with.
    pub fn transport(&self) -> &dyn Transport {
        self.shared.transport.as_ref()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.closed = true;
        self.shared.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::transport::Reply;

    /// A transport that is never asked: these tests hold no key.
    struct Never;

    impl Transport for Never {
        fn post(&self, _: &Key, _: &str) -> Result<Reply, Error> {
            Err(Error::Unreachable("not asked".to_owned()))
        }
    }

    fn session() -> Session {
        Session::new(
            Arc::new(Never),
            Paths {
                config: None,
                state: None,
            },
            None,
            Duration::from_secs(3600),
        )
    }

    #[test]
    fn a_view_call_told_late_does_not_undo_a_later_one_of_the_same_member() {
        let session = session();
        session.set_view(Some(2), None, Some("rate".to_owned()), None);
        session.set_view(Some(1), None, Some("rat".to_owned()), None);
        assert_eq!(session.state().view.filter, "rate");
        session.set_view(Some(3), None, Some("rates".to_owned()), None);
        assert_eq!(session.state().view.filter, "rates");
    }

    #[test]
    fn a_call_is_held_to_the_order_only_of_the_members_it_sets() {
        let session = session();
        // The filter call has the higher number and comes first: the scope call is not its.
        session.set_view(Some(5), None, Some("bug".to_owned()), None);
        session.set_view(Some(4), Some(Scope::All), None, None);
        let folded = HashMap::from([("todo".to_owned(), true)]);
        session.set_view(Some(3), None, None, Some(folded.clone()));
        let state = session.state();
        assert_eq!(state.view.scope, Scope::All);
        assert_eq!(state.view.filter, "bug");
        assert_eq!(state.view.closed, folded);
    }

    #[test]
    fn a_page_that_begins_again_is_heard_from_its_first_call() {
        let session = session();
        session.set_view(Some(9), None, Some("old".to_owned()), None);
        session.forget_view_order();
        session.set_view(Some(1), None, Some("new".to_owned()), None);
        assert_eq!(session.state().view.filter, "new");
        // A call with no number is the latest.
        session.set_view(None, None, Some("last".to_owned()), None);
        assert_eq!(session.state().view.filter, "last");
    }
}
