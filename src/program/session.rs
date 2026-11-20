//! What the program holds for the page: the key, the lists as Linear last said them (My issues, and
//! the one list the page looks at where it is another or is narrowed by filters, or is a search), the
//! names of the lists a person can go to, what the page is looking at (the scope, the filters Linear
//! narrows it by, the words of the filter, the groups the person folded), and the worker
//! that asks Linear again, at the page's word and every few minutes.
//!
//! The worker is one thread. It asks for My issues and for the list the page looks at, one after the
//! other, and puts them in `State` as a whole; a failure leaves the lists as they were and says why.
//! Then it asks for the names of the lists, which are put in `State` apart: a list can be shown
//! without them. A page that goes to another list has it asked for (`set_view`). Whoever changes
//! `State` bumps `generation` and wakes the topic's threads and the worker.
//!
//! What the program writes is a new issue and the changes of issues. `create` sends once and, where
//! no answer came, asks for the issue by the id of the draft before it says it failed. `change` sends
//! once, writes what Linear answers into the lists and the issues kept (the lists are not asked for
//! again), and where no answer came asks for the issues it named, one by one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::linear::edit::{self, Target};
use crate::linear::types::Comment;
use crate::linear::{
    self, Change, Closed, Created, Entry, Error, FilterOptions, Filters, Issue, Key, Listing,
    NewIssue, Options, Scope, Team, Transport, Viewer,
};
use crate::pictures::{Document, Given, Pictures, Refusal, Saved, SendFailed, Shown, Unread, Why};
use crate::state::Paths;
use crate::state::credentials::{self, Problem, Source};
use crate::work::runner::{Runner, System};
use crate::work::{Setup, bateri, handoff};

/// How long the lists are left before they are asked for again, while the page is open.
pub const REFRESH_EVERY: Duration = Duration::from_secs(300);

/// How many opened issues are kept before they are let go of.
const DETAILS_KEPT: usize = 64;
/// The most edits whose texts are held at once: an edit begun before the most were let go is begun
/// again (`NOT_BEGUN`).
const EDITS_KEPT: usize = 64;

/// What `edit_save` says of an edit whose text is not held (it was never read, or was let go).
pub const NOT_BEGUN: &str = "the edit was not begun here";

/// The agent an issue was handed to, and the pane it works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Working {
    pub agent: &'static str,
    pub pane: String,
    /// The pane has written nothing for `bateri::IDLE_AFTER`: the agent is done, or waits for the
    /// person (they cannot be told apart). Only the poll of the panes sets it.
    pub idle: bool,
}

/// What the program needs to hand issues to an agent: what the machine says, and what runs the
/// commands.
pub struct Work {
    pub setup: Setup,
    pub runner: Arc<dyn Runner>,
    /// How often the panes of the agents are looked at, while there are any.
    pub poll: Duration,
}

/// How often the panes of the agents are looked at.
const POLL: Duration = Duration::from_secs(3);

/// What the page is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub scope: Scope,
    /// What the list is narrowed by in Linear (status, priority, assignee, labels, project). Not the
    /// words of `filter`, which narrow what is held.
    pub filters: Filters,
    pub filter: String,
    /// The groups the person folded (`true`) or opened (`false`), by key; one not here is as it
    /// begins (backlog and triage folded).
    pub closed: HashMap<String, bool>,
}

/// The lists, and whose they are: the person's own, always (all of them: it is what is counted), and
/// the one the page looks at where it is another or is narrowed by filters (with the scope and the
/// filters it was asked for).
#[derive(Debug, Clone, PartialEq)]
pub struct Lists {
    pub viewer: Viewer,
    pub mine: Listing,
    pub shown: Option<(Scope, Listing)>,
    /// The filters `shown` was asked for with.
    pub filters: Filters,
}

/// Whether a list is asked for apart from My issues: it is another list, or it is narrowed.
#[must_use]
pub fn is_apart(scope: &Scope, filters: &Filters) -> bool {
    *scope != Scope::Mine || !filters.is_empty()
}

impl Lists {
    /// The list of `scope` narrowed by `filters`, where it is held.
    #[must_use]
    pub fn of(&self, scope: &Scope, filters: &Filters) -> Option<&Listing> {
        if !is_apart(scope, filters) {
            return Some(&self.mine);
        }
        match &self.shown {
            Some((held, listing)) if held == scope && self.filters == *filters => Some(listing),
            _ => None,
        }
    }
}

/// A pane that is watched: the agent in it, the issues it was given, and the file of them it reads.
#[derive(Debug)]
struct Held {
    pane: String,
    agent: &'static str,
    ids: Vec<String>,
    context: PathBuf,
}

#[derive(Debug)]
pub struct State {
    pub key: Option<Key>,
    pub source: Option<Source>,
    /// Why the key there is cannot be used (a key that is not one, a file that cannot be read).
    pub problem: Option<String>,
    pub lists: Option<Lists>,
    /// The lists a person can go to, by name, as Linear last said them; and why they could not be
    /// asked for the last time, where they could not.
    pub names: Option<Vec<Entry>>,
    pub names_error: Option<Error>,
    /// The worker is asking Linear now.
    pub refreshing: bool,
    /// What went wrong the last time Linear was asked.
    pub error: Option<Error>,
    /// When the lists were told, seconds since the epoch.
    pub updated: Option<u64>,
    pub view: View,
    /// The number of the last `view` call that set each member of `view` (scope, filters, filter, closed),
    /// which are made at the same time and may be told out of their order.
    view_seen: [u64; 4],
    /// The issues an agent was started on whose pane has not ended, by Linear's id.
    pub working: HashMap<String, Working>,
    /// The panes that are watched, each with what its agent was given and the file it reads from,
    /// which is not put away while the pane is on. Apart from `working`, which has one entry for each
    /// issue: an issue given to another agent while the first is on has two panes.
    held: Vec<Held>,
    /// Issues opened, as Linear told them (with their description and comments), by id: they are
    /// kept until the lists are asked for again with everything (`r`, the five minutes, an issue
    /// made), so that an issue seen again is not asked for again in between.
    details: HashMap<String, Issue>,
    /// The texts the edits the person began are of, by the issue (as the page named it) and what is
    /// edited: what Linear held when each was read for the edit, which it must hold still for the
    /// edit to be written (`edit_save`).
    edits: HashMap<(String, Target), String>,
    /// The teams an issue can be made in, and what each offers, as Linear last said: they are let go
    /// whenever the lists are asked for again.
    teams: Option<Vec<Team>>,
    choices: HashMap<String, Options>,
    /// What the filters offer, as Linear last said: let go whenever the lists are asked for again.
    filter_options: Option<FilterOptions>,
    generation: u64,
    refresh_wanted: bool,
    /// What is wanted is the asking of everything (`r`, the five minutes, a key that was just
    /// given), the names of the lists and the choices of a new issue too; not only the list that the
    /// page went to.
    everything: bool,
    /// An issue was changed while the lists were on their way: what comes is from before it, and is
    /// let go.
    stale: bool,
    worker: bool,
    /// A thread is looking at the panes of the agents (it stops when there are none).
    poller: bool,
    closed: bool,
}

impl State {
    /// The pane `pane` has ended: its file may be put away, and each issue whose mark was its own
    /// takes the mark of another pane that is still on and was given it too, or loses its mark. (The
    /// mark of an issue is the last pane it was given to; when that one ends first, the issue is not
    /// left without one while the other agent still works on it.)
    fn pane_ended(&mut self, pane: &str) {
        self.held.retain(|held| held.pane != pane);
        let marked: Vec<String> = self
            .working
            .iter()
            .filter(|(_, working)| working.pane == pane)
            .map(|(id, _)| id.clone())
            .collect();
        for id in marked {
            match self.held.iter().rev().find(|held| held.ids.contains(&id)) {
                Some(other) => {
                    let working = Working {
                        agent: other.agent,
                        pane: other.pane.clone(),
                        idle: false,
                    };
                    self.working.insert(id, working);
                }
                None => {
                    self.working.remove(&id);
                }
            }
        }
    }

    /// Tells that an issue of the lists was changed: where the lists are on their way, they were
    /// asked for before it, so what comes is let go and they are asked for once more.
    fn note_change(&mut self) {
        if self.refreshing {
            self.stale = true;
            self.refresh_wanted = true;
        }
    }

    /// What the worker does with the lists it asked for, or with why it could not. A list that the
    /// page does not look at any more is let go of, and the one it looks at now is asked for.
    fn landed(&mut self, got: Result<Lists, Error>) {
        self.refreshing = false;
        match got {
            // Asked for before a change was made: it would put the old values back. The change
            // asked for the lists once more (`note_change`).
            Ok(_) if self.stale => {}
            Ok(mut lists) => {
                let wanted = self.view.scope.clone();
                // (A list asked for with other filters than the page has now is not the one it wants.)
                lists.shown = lists
                    .shown
                    .filter(|(held, _)| *held == wanted && lists.filters == self.view.filters);
                if is_apart(&wanted, &self.view.filters) && lists.shown.is_none() {
                    self.refresh_wanted = true;
                }
                self.lists = Some(lists);
                self.error = None;
                self.updated = Some(now_seconds());
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// What the worker does with the names of the lists, or with why it could not ask for them: the
    /// ones there were stay.
    fn named(&mut self, got: Result<Vec<Entry>, Error>) {
        match got {
            Ok(names) => {
                self.names = Some(names);
                self.names_error = None;
            }
            Err(error) => self.names_error = Some(error),
        }
    }

    /// Whether the worker asks for the names of the lists and the choices with the lists: it was asked
    /// to (`r`), it is `due`, or the names were never asked for. Names that were asked for and did not
    /// come are asked for again at `r` and the five minutes, not with every list the page goes to: a
    /// list that Linear will not name would otherwise have four more questions with each.
    fn asks_for_everything(&self, due: bool) -> bool {
        self.everything || due || (self.names.is_none() && self.names_error.is_none())
    }

    /// The worker is about to ask for the lists (it has been asked to, and has not begun).
    #[must_use]
    pub fn refresh_wanted_now(&self) -> bool {
        self.refresh_wanted
    }

    /// Lets go of the teams and what each offers, and of the issues opened, so that the next
    /// question of each asks Linear again.
    pub fn forget_kept(&mut self) {
        self.teams = None;
        self.choices.clear();
        self.filter_options = None;
        self.details.clear();
    }

    /// Writes an issue as Linear says it is now into what is held: the person's list has it where it
    /// is theirs, and not where it is not; All issues has it; any other list the page looks at keeps
    /// an issue it holds as it is now, done or not (it cannot say whether Linear's own rule for that
    /// list takes a new one). The issue kept with its description is as it is now. Gives whether the
    /// list the page is looking at holds it.
    fn put(&mut self, issue: &Issue) -> bool {
        let mine = issue.assignee.as_ref().is_some_and(|who| who.is_me);
        let mut listed = false;
        if let Some(lists) = &mut self.lists {
            let row = Issue {
                description: None,
                comments: None,
                ..issue.clone()
            };
            place(&mut lists.mine, &row, mine);
            let narrowed = !lists.filters.is_empty();
            if let Some((scope, listing)) = &mut lists.shown {
                let held = listing.issues.iter().any(|held| held.id == row.id);
                // The person's own list holds what is theirs.
                let wanted = match scope {
                    Scope::Mine => mine && held,
                    Scope::All => held || !narrowed,
                    _ => held,
                };
                place(listing, &row, wanted);
            }
            listed = lists
                .of(&self.view.scope, &self.view.filters)
                .is_some_and(|listing| listing.issues.iter().any(|held| held.id == row.id));
        }
        // What is kept is by the id it was asked for by, which may be the identifier.
        for kept in self.details.values_mut().filter(|kept| kept.id == issue.id) {
            let held = kept.clone();
            *kept = issue.clone();
            // What a change answers is a row: the issue it is of keeps what only the issue asked for
            // alone has, its text and the issues around it, which no change touches.
            if issue.comments.is_none() {
                kept.description = held.description;
                kept.comments = held.comments;
                kept.parent = held.parent;
                kept.children = held.children;
                kept.relations = held.relations;
                kept.inverse_relations = held.inverse_relations;
            }
        }
        match self.view.scope {
            // (A list narrowed by filters cannot say whether Linear's filter takes what changed: it
            // holds what it held.)
            _ if !self.view.filters.is_empty() => listed,
            Scope::Mine => mine,
            Scope::All => true,
            _ => listed,
        }
    }
}

/// `issue` in `listing` where it is `wanted`, in the place it was; out of it where it is not. One
/// that is wanted and was not there is the newest, so it goes first.
fn place(listing: &mut Listing, issue: &Issue, wanted: bool) {
    let at = listing.issues.iter().position(|held| held.id == issue.id);
    match (at, wanted) {
        (Some(at), true) => listing.issues[at] = issue.clone(),
        (Some(at), false) => {
            listing.issues.remove(at);
        }
        (None, true) => listing.issues.insert(0, issue.clone()),
        (None, false) => {}
    }
}

/// An issue a change was made to, and whether the list the page is looking at holds it still (one
/// that is done, or is another's in My issues, is no longer in it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub id: String,
    pub listed: bool,
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
    /// The files of Linear the page draws or saves and the ones the person gives it.
    pub pictures: Pictures,
    /// The program's hand on the view: what holds the files the page draws (`serve` is given it).
    control: pencere::ViewControl,
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

/// Looks at the panes that are watched, as often as `every`, and marks the issues of those that have
/// written nothing for a while; ends where there is no pane to look at. Only marks: what the list
/// cannot say (bateri does not answer, the pane is not in its list, no time is given of it) is that
/// the agent is at work, as it was before, and no mark is ever taken away here.
fn look_at_panes(shared: &Shared, runner: &dyn Runner, every: Duration) {
    loop {
        {
            let mut state = shared.lock();
            if state.closed || state.working.is_empty() {
                state.poller = false;
                return;
            }
        }
        let seen = bateri::output_times(runner);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |since| since.as_secs_f64());
        {
            let mut state = shared.lock();
            let mut changed = false;
            for working in state.working.values_mut() {
                let idle = seen
                    .as_ref()
                    .and_then(|seen| seen.get(&working.pane))
                    .is_some_and(|at| bateri::idle(*at, now));
                if working.idle != idle {
                    working.idle = idle;
                    changed = true;
                }
            }
            if changed {
                shared.changed(&mut state);
            }
        }
        thread::sleep(every);
    }
}

/// Asks Linear for My issues, and for the list the page looks at where it is another or is narrowed
/// by `filters`.
fn fetch_lists(
    transport: &dyn Transport,
    key: &Key,
    shown: &Scope,
    filters: &Filters,
) -> Result<Lists, Error> {
    let mine = linear::fetch(
        transport,
        key,
        &Scope::Mine,
        &Filters::default(),
        Closed::Recent,
    )?;
    let held = if is_apart(shown, filters) {
        Some((
            shown.clone(),
            linear::fetch(transport, key, shown, filters, Closed::Recent)?.listing,
        ))
    } else {
        None
    };
    Ok(Lists {
        viewer: mine.viewer,
        mine: mine.listing,
        shown: held,
        filters: filters.clone(),
    })
}

/// The worker: asks when it is asked to, and again when `every` has passed.
fn work(shared: &Shared) {
    let mut next = Instant::now() + shared.every;
    loop {
        let (key, scope, filters, everything) = {
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
            // The names of the lists, the choices of a new issue and the issues opened are asked for
            // with everything (the first time, `r`, the five minutes); going to another list asks for
            // that list only.
            let everything = state.asks_for_everything(Instant::now() >= next);
            state.refresh_wanted = false;
            state.everything = false;
            state.refreshing = true;
            state.stale = false;
            if everything {
                state.forget_kept();
            }
            shared.changed(&mut state);
            (
                state.key.clone(),
                state.view.scope.clone(),
                state.view.filters.clone(),
                everything,
            )
        };
        let Some(key) = key else { continue };
        let got = fetch_lists(shared.transport.as_ref(), &key, &scope, &filters);
        {
            let mut state = shared.lock();
            state.landed(got);
            shared.changed(&mut state);
        }
        if everything {
            // The names of the lists come after the lists: a list is shown without them.
            let named = linear::names(shared.transport.as_ref(), &key);
            let mut state = shared.lock();
            state.named(named);
            shared.changed(&mut state);
            next = Instant::now() + shared.every;
        }
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
                names: None,
                names_error: None,
                refreshing: false,
                error: None,
                updated: None,
                view: View {
                    scope: Scope::Mine,
                    filters: Filters::default(),
                    filter: String::new(),
                    closed: HashMap::new(),
                },
                working: HashMap::new(),
                held: Vec::new(),
                details: HashMap::new(),
                edits: HashMap::new(),
                teams: None,
                choices: HashMap::new(),
                filter_options: None,
                view_seen: [0; 4],
                generation: 0,
                refresh_wanted: false,
                everything: false,
                stale: false,
                worker: false,
                poller: false,
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
            poll: POLL,
        };
        let control = pencere::ViewControl::new();
        Self {
            shared,
            paths,
            work,
            pictures: Pictures::new(Arc::new(control.clone())),
            control,
        }
    }

    /// The hand on the view that `serve` is given (`Options::control`): the pictures are attached
    /// and the files the person gave are read through it.
    #[must_use]
    pub fn control(&self) -> pencere::ViewControl {
        self.control.clone()
    }

    /// The session with what the machine says and what runs the commands (`bateri`, `git`).
    #[must_use]
    pub fn with_work(mut self, setup: Setup, runner: Arc<dyn Runner>) -> Self {
        self.work = Work {
            setup,
            runner,
            poll: POLL,
        };
        self
    }

    /// The same, with the panes looked at as often as `poll` (a test does not wait for seconds).
    #[cfg(test)]
    #[must_use]
    pub fn with_poll(mut self, poll: Duration) -> Self {
        self.work.poll = poll;
        self
    }

    /// An agent was started on the issues `ids`, in `pane`: they carry its mark until the pane's
    /// program ends, which a thread waits for.
    pub fn watch(&self, ids: &[String], agent: &'static str, pane: &str, context: &Path) {
        {
            let mut state = self.shared.lock();
            for id in ids {
                state.working.insert(
                    id.clone(),
                    Working {
                        agent,
                        pane: pane.to_owned(),
                        idle: false,
                    },
                );
            }
            state.held.push(Held {
                pane: pane.to_owned(),
                agent,
                ids: ids.to_vec(),
                context: context.to_path_buf(),
            });
            self.shared.changed(&mut state);
        }
        self.poll();
        let shared = Arc::clone(&self.shared);
        let runner = Arc::clone(&self.work.runner);
        let pane = pane.to_owned();
        let waited = pane.clone();
        let started = thread::Builder::new()
            .name("linearpen-wait".to_owned())
            .spawn(move || {
                bateri::wait(runner.as_ref(), &waited);
                let mut state = shared.lock();
                state.pane_ended(&waited);
                shared.changed(&mut state);
            });
        if started.is_err() {
            // Nothing will say when it ends: no mark is better than one that stays.
            let mut state = self.shared.lock();
            state.pane_ended(&pane);
            self.shared.changed(&mut state);
        }
    }

    /// Puts away the folders of old runs in the state folder (`handoff::prune`), except those of the
    /// panes that are watched. Says how many went.
    pub fn tidy(&self) -> usize {
        let Some(state) = self.paths.state.as_deref() else {
            return 0;
        };
        let in_use: Vec<PathBuf> = self
            .shared
            .lock()
            .held
            .iter()
            .map(|held| held.context.clone())
            .collect();
        handoff::prune(state, now_seconds(), &in_use)
    }

    /// Looks at the panes of the agents from now on, every `Work::poll`, until there is none: one
    /// thread for all of them, and none where nothing is watched.
    fn poll(&self) {
        {
            let mut state = self.shared.lock();
            if state.poller {
                return;
            }
            state.poller = true;
        }
        let shared = Arc::clone(&self.shared);
        let runner = Arc::clone(&self.work.runner);
        let every = self.work.poll;
        let started = thread::Builder::new()
            .name("linearpen-poll".to_owned())
            .spawn(move || look_at_panes(&shared, runner.as_ref(), every));
        if started.is_err() {
            // The marks go on saying the agent is at work, which is all they said before.
            self.shared.lock().poller = false;
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
            state.everything = true;
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

    /// The key to ask Linear with.
    fn key(&self) -> Result<Key, Error> {
        self.shared
            .lock()
            .key
            .clone()
            .ok_or_else(|| Error::Failed("there is no key to ask Linear with".to_owned()))
    }

    /// The teams an issue can be made in: as they were kept, else asked of Linear (without holding
    /// the state while it is).
    pub fn teams(&self) -> Result<Vec<Team>, Error> {
        if let Some(kept) = self.shared.lock().teams.clone() {
            return Ok(kept);
        }
        let teams = linear::teams(self.transport(), &self.key()?)?;
        self.shared.lock().teams = Some(teams.clone());
        Ok(teams)
    }

    /// What a new issue of the team with Linear's id `team` can be given: as it was kept, else asked.
    pub fn options(&self, team: &str) -> Result<Options, Error> {
        if let Some(kept) = self.shared.lock().choices.get(team) {
            return Ok(kept.clone());
        }
        let options = linear::options(self.transport(), &self.key()?, team)?;
        self.shared
            .lock()
            .choices
            .insert(team.to_owned(), options.clone());
        Ok(options)
    }

    /// What the filters offer to choose from (the states, the people, the labels): as it was kept,
    /// else asked.
    pub fn filter_options(&self) -> Result<FilterOptions, Error> {
        if let Some(kept) = self.shared.lock().filter_options.clone() {
            return Ok(kept);
        }
        let options = linear::filter_options(self.transport(), &self.key()?)?;
        self.shared.lock().filter_options = Some(options.clone());
        Ok(options)
    }

    /// Makes the issue of the draft in Linear: the program's one write. It is sent once. Where it
    /// did not come back whole (no answer, or an error), the issue is asked for by the id of the
    /// draft, which Linear took as the issue's own: if it is there it was made, and that is the
    /// answer; if Linear says there is none, the failure is told; if the asking comes to nothing too,
    /// it is not known (`Error::Unsure`). Nothing is sent again unless the person asks again, with
    /// the same id. A key Linear does not take cannot have made it, and is not asked about.
    pub fn create(&self, input: &NewIssue) -> Result<Created, Error> {
        let key = self.key()?;
        let transport = self.transport();
        let failure = match linear::create(transport, &key, input) {
            Ok(created) => {
                self.refresh();
                return Ok(created);
            }
            Err(failure @ (Error::Refused | Error::Forbidden)) => return Err(failure),
            Err(failure) => failure,
        };
        match linear::made(transport, &key, &input.id) {
            Ok(Some(created)) => {
                self.refresh();
                Ok(created)
            }
            // Linear has no such issue, or has another: the draft was not made.
            Ok(None) => Err(failure),
            Err(_) => Err(Error::Unsure),
        }
    }

    /// Makes `change` in the issues with Linear's ids `ids`, from one to fifty: the program's other
    /// write. It is sent once. What Linear answers is written into the lists and the issues kept as
    /// it is, and the topic is told; no list is asked for again. Where it did not come back whole
    /// (no answer), or came back without one of the issues, each of those is asked for, so that what
    /// is shown is what Linear holds; nothing is sent again, and the failure is told. An answer that is
    /// an error (Linear said no, or the key is not taken) says the change was not made, and nothing is
    /// asked about.
    pub fn change(&self, ids: &[String], change: &Change) -> Result<Vec<Changed>, Error> {
        if ids.is_empty() || ids.len() > linear::BATCH_MOST {
            return Err(Error::Failed(format!(
                "from one to {} issues are changed at once",
                linear::BATCH_MOST
            )));
        }
        let key = self.key()?;
        match linear::change(self.transport(), &key, ids, change) {
            Ok(issues) => {
                let missing: Vec<String> = ids
                    .iter()
                    .filter(|id| !issues.iter().any(|got| got.id.eq_ignore_ascii_case(id)))
                    .cloned()
                    .collect();
                let mut told = Vec::new();
                {
                    let mut state = self.shared.lock();
                    for issue in &issues {
                        let listed = state.put(issue);
                        told.push(Changed {
                            id: issue.id.clone(),
                            listed,
                        });
                    }
                    state.note_change();
                    self.shared.changed(&mut state);
                }
                told.extend(self.ask_again(&key, &missing));
                Ok(told)
            }
            Err(failure @ (Error::Refused | Error::Forbidden)) => Err(failure),
            // No answer came: the change may have been made, and Linear is asked what it holds.
            Err(failure @ Error::Unreachable(_)) => {
                self.ask_again(&key, ids);
                Err(failure)
            }
            // What came is not an answer to read (a gateway's 502, a body cut short): it says
            // nothing of whether the change was made, so Linear is asked as well, and it is told
            // that it is not known.
            Err(Error::Failed(why)) if linear::left_open(&why) => {
                self.ask_again(&key, ids);
                Err(Error::Unsure)
            }
            // Linear answered and said no: nothing is sent again and nothing more is asked.
            Err(failure) => Err(failure),
        }
    }

    /// Posts `body` as a comment of the issue `issue` (Linear's id of it, or its identifier) under
    /// `id`, a UUID the caller made: the program's write of a comment. It is sent once, whatever comes back.
    /// What Linear answers is put among the comments of the issue kept, so that the issue asked for
    /// next has it. Where it did not come back whole (no answer, or what cannot be read) the issue is
    /// asked for, so that what is shown is what Linear holds, and the failure is told (not known, for
    /// an answer that cannot be read); nothing is sent again. An answer that is an error (Linear said
    /// no, or the key is not taken) says it was not posted, and nothing is asked about.
    pub fn comment(&self, id: &str, issue: &str, body: &str) -> Result<Comment, Error> {
        let key = self.key()?;
        match linear::comment(self.transport(), &key, id, issue, body) {
            Ok(posted) => {
                let mut state = self.shared.lock();
                for kept in state
                    .details
                    .values_mut()
                    .filter(|kept| kept.id == issue || kept.identifier == issue)
                {
                    if let Some(comments) = &mut kept.comments {
                        comments.nodes.push(posted.clone());
                    }
                }
                Ok(posted)
            }
            Err(failure @ (Error::Refused | Error::Forbidden)) => Err(failure),
            Err(failure @ Error::Unreachable(_)) => {
                let _ = self.detail_fresh(issue);
                Err(failure)
            }
            Err(Error::Failed(why)) if linear::left_open(&why) => {
                let _ = self.detail_fresh(issue);
                Err(Error::Unsure)
            }
            Err(failure) => Err(failure),
        }
    }

    /// The picture or PDF at `src` (an address of Linear's files) for `owner`: fetched with the key, given
    /// to the terminal, and told how large it is; or why it is not drawn.
    pub fn picture(&self, src: &str, owner: &str) -> Result<Shown, Refusal> {
        let key = self.key().map_err(|_| Refusal::from(Why::Unreachable))?;
        self.pictures.show(self.transport(), &key, src, owner)
    }

    /// What is held for `owner` (an issue) is let go.
    pub fn release(&self, owner: &str) {
        self.pictures.release(owner);
    }

    /// Reads the PDF at `src` (an address of Linear's files) for its pages and words, for `owner`.
    pub fn open_pdf(&self, src: &str, owner: &str) -> Result<Arc<Document>, Unread> {
        let key = self.key().map_err(|_| Unread::Unreachable)?;
        self.pictures.open_pdf(self.transport(), &key, src, owner)
    }

    /// The PDF read for `owner`, if one is.
    pub fn document(&self, owner: &str) -> Option<Arc<Document>> {
        self.pictures.document(owner)
    }

    /// The PDF read for `owner` is let go.
    pub fn close_pdf(&self, owner: &str) {
        self.pictures.close_pdf(owner);
    }

    /// Saves the file at `src` (an address of Linear's files) where the person chose, by `handle`: fetched
    /// with the key into a copy of the program's own and written to the place; how many bytes.
    pub fn save(&self, src: &str, handle: &str) -> Result<u64, Saved> {
        let key = self.key().map_err(|_| Saved::Unreachable)?;
        self.pictures.save(self.transport(), &key, src, handle)
    }

    /// Puts the files the person gave in Linear's storage and gives their Markdown lines; the first
    /// that cannot be sent ends it, and nothing is sent of what was to follow.
    pub fn send_pictures(&self, given: &[Given], what: &str) -> Result<Vec<String>, SendFailed> {
        let key = self.key().map_err(|_| SendFailed::Refused)?;
        self.pictures.send(self.transport(), &key, given, what)
    }

    /// Adds the lines of files at the end of the description of the issue `issue` (Linear's id of it,
    /// or its identifier): the program's one write of a description. The description is read whole from
    /// Linear now, and what is sent is it and the lines. Where the reading failed, nothing was sent. The
    /// write is sent once; where no answer came, or what came cannot be read, the issue is asked for, so
    /// that what is shown is what Linear holds, and the failure is told (not known, for an answer that
    /// cannot be read). An answer that is an error says it was not written, and nothing is asked.
    pub fn describe(&self, issue: &str, lines: &[String]) -> Result<(), linear::Step> {
        let key = self.key().map_err(linear::Step::Read)?;
        match linear::add_files(self.transport(), &key, issue, lines) {
            Ok(_) => {
                // What Linear holds now, with the files in it, is what the issue kept tells.
                let _ = self.detail_fresh(issue);
                Ok(())
            }
            Err(before @ (linear::Step::Read(_) | linear::Step::TooLong)) => Err(before),
            Err(linear::Step::Write(failure @ (Error::Refused | Error::Forbidden))) => {
                Err(linear::Step::Write(failure))
            }
            Err(linear::Step::Write(failure @ Error::Unreachable(_))) => {
                let _ = self.detail_fresh(issue);
                Err(linear::Step::Write(failure))
            }
            Err(linear::Step::Write(Error::Failed(why))) if linear::left_open(&why) => {
                let _ = self.detail_fresh(issue);
                Err(linear::Step::Write(Error::Unsure))
            }
            Err(linear::Step::Write(failure)) => Err(linear::Step::Write(failure)),
        }
    }

    /// What is edited (the description of the issue `issue`, Linear's id of it or its identifier, or a
    /// comment of the person's on it) as Linear holds it now, whole: the text an edit begins from. It is
    /// held, so that the edit is written only where Linear holds it still (`edit_save`); a second read
    /// of the same takes its place.
    pub fn edit_open(&self, issue: &str, target: &Target) -> Result<String, Error> {
        let key = self.key()?;
        let now = edit::current(self.transport(), &key, issue, target)?;
        let mut state = self.shared.lock();
        if state.edits.len() >= EDITS_KEPT {
            state.edits.clear();
        }
        state
            .edits
            .insert((issue.to_owned(), target.clone()), now.text.clone());
        Ok(now.text)
    }

    /// Writes `text` over what is edited, where Linear holds what the edit began from (`edit_open`):
    /// the program's write of the words of a description, and of a comment. It is read again first and
    /// sent once. Where Linear holds another text, nothing is sent: what it holds is the text the edit
    /// is checked against from here (the person, told, saves again to put theirs in its place). The
    /// issue as Linear holds it after is asked for and kept, and given (none where it could not be).
    /// Where the write was sent and no answer came, or what came cannot be read, the issue is asked for
    /// too and the failure is told (not known, for an answer that cannot be read): saving again is
    /// safe, for a text Linear holds already is not sent.
    pub fn edit_save(
        &self,
        issue: &str,
        target: &Target,
        text: &str,
    ) -> Result<Option<Issue>, edit::Step> {
        let key = self.key().map_err(edit::Step::Read)?;
        let held = (issue.to_owned(), target.clone());
        let base = self
            .shared
            .lock()
            .edits
            .get(&held)
            .cloned()
            .ok_or_else(|| edit::Step::Read(Error::Failed(NOT_BEGUN.to_owned())))?;
        match edit::save(self.transport(), &key, issue, target, &base, text) {
            Ok(()) => {
                self.shared.lock().edits.remove(&held);
                Ok(self.detail_fresh(issue).ok())
            }
            Err(edit::Step::Changed(now)) => {
                self.shared.lock().edits.insert(held, now.clone());
                let _ = self.detail_fresh(issue);
                Err(edit::Step::Changed(now))
            }
            Err(edit::Step::Write(failure @ Error::Unreachable(_))) => {
                let _ = self.detail_fresh(issue);
                Err(edit::Step::Write(failure))
            }
            Err(edit::Step::Write(Error::Failed(why))) if linear::left_open(&why) => {
                let _ = self.detail_fresh(issue);
                Err(edit::Step::Write(Error::Unsure))
            }
            Err(other) => Err(other),
        }
    }

    /// Asks Linear for each of the issues `ids`, one after the other, and writes what it says into
    /// what is held. It stops at the first that cannot be reached: the others would not be either.
    fn ask_again(&self, key: &Key, ids: &[String]) -> Vec<Changed> {
        let mut told = Vec::new();
        for id in ids {
            match linear::fetch_issue(self.transport(), key, id) {
                Ok(issue) => {
                    let mut state = self.shared.lock();
                    let listed = state.put(&issue);
                    state.note_change();
                    self.shared.changed(&mut state);
                    told.push(Changed {
                        id: issue.id,
                        listed,
                    });
                }
                Err(Error::Unreachable(_) | Error::Refused | Error::Forbidden) => break,
                Err(_) => {}
            }
        }
        told
    }

    /// A page that begins again numbers its `view` calls from the start: what was held of the last
    /// one's numbers is let go.
    pub fn forget_view_order(&self) {
        self.shared.lock().view_seen = [0; 4];
    }

    /// Changes what the page is looking at, and wakes whoever shows it. The page numbers its calls
    /// (`seq`): a member that a call with a higher number has set is left as it is, for calls run at
    /// the same time and may come out of their order. A call with no number is the latest.
    pub fn set_view(
        &self,
        seq: Option<u64>,
        scope: Option<Scope>,
        filters: Option<Filters>,
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
        let (scope_ok, filters_ok, filter_ok, closed_ok) = (
            allowed(0, scope.is_some()),
            allowed(1, filters.is_some()),
            allowed(2, filter.is_some()),
            allowed(3, closed.is_some()),
        );
        let scope = scope.filter(|scope| scope_ok && state.view.scope != *scope);
        let filters = filters.filter(|filters| filters_ok && state.view.filters != *filters);
        if scope.is_some() || filters.is_some() {
            // A list the page left (or that other filters narrowed) is let go of; the one it goes to
            // is asked for, with My issues (a list is shown as Linear says it now).
            if let Some(lists) = &mut state.lists {
                lists.shown = None;
            }
            if let Some(scope) = scope {
                state.view.scope = scope;
            }
            if let Some(filters) = filters {
                state.view.filters = filters;
            }
            // (Where the last asking failed, even My issues is asked for again: the page that goes back to
            // it from a list that failed must not be left under the error of that list.)
            if is_apart(&state.view.scope, &state.view.filters) || state.error.is_some() {
                state.refresh_wanted = true;
            }
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
    use crate::linear::types::StateKind;

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

    /// The state as `watch` leaves it when the issue `issue-412` was given to the agent of `p9` and
    /// then to the agent of `p10`: one mark, the later pane's, and both panes held.
    fn given_twice(session: &Session) {
        let mut state = session.shared.lock();
        for (pane, agent) in [("p9", "Claude"), ("p10", "Codex")] {
            let working = Working {
                agent,
                pane: pane.to_owned(),
                idle: false,
            };
            state.working.insert("issue-412".to_owned(), working);
            state.held.push(Held {
                pane: pane.to_owned(),
                agent,
                ids: vec!["issue-412".to_owned()],
                context: PathBuf::from(format!("/runs/{pane}/issues.md")),
            });
        }
    }

    #[test]
    fn an_issue_given_to_two_agents_keeps_a_mark_until_both_their_panes_have_ended() {
        // The later pane ends first: the mark goes to the pane that is still on.
        let later_first = session();
        given_twice(&later_first);
        let mut state = later_first.shared.lock();
        state.pane_ended("p10");
        let working = &state.working["issue-412"];
        assert_eq!((working.pane.as_str(), working.agent), ("p9", "Claude"));
        assert_eq!(state.held.len(), 1);
        state.pane_ended("p9");
        assert!(state.working.is_empty() && state.held.is_empty());
        drop(state);
        // The earlier pane ends first: the mark stays the later pane's.
        let earlier_first = session();
        given_twice(&earlier_first);
        let mut state = earlier_first.shared.lock();
        state.pane_ended("p9");
        let working = &state.working["issue-412"];
        assert_eq!((working.pane.as_str(), working.agent), ("p10", "Codex"));
        state.pane_ended("p10");
        assert!(state.working.is_empty() && state.held.is_empty());
    }

    use std::collections::VecDeque;

    use crate::linear::{Created, NewIssue, Priority};

    /// Answers from a list, one to each request, and keeps the bodies it was sent.
    struct Scripted {
        replies: Mutex<VecDeque<Result<Reply, Error>>>,
        sent: Arc<Mutex<Vec<String>>>,
    }

    impl Transport for Scripted {
        fn post(&self, _: &Key, body: &str) -> Result<Reply, Error> {
            self.sent.lock().unwrap().push(body.to_owned());
            self.replies
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| Err(Error::Unreachable("no more replies".to_owned())))
        }
    }

    /// A session with a key that is answered from `replies`; and what it sends.
    fn answered(replies: Vec<Result<Reply, Error>>) -> (Session, Arc<Mutex<Vec<String>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let transport = Scripted {
            replies: Mutex::new(replies.into()),
            sent: Arc::clone(&sent),
        };
        let session = Session::new(
            Arc::new(transport),
            Paths {
                config: None,
                state: None,
            },
            Some("lin_api_test"),
            Duration::from_secs(3600),
        );
        (session, sent)
    }

    fn said(body: &str) -> Result<Reply, Error> {
        Ok(Reply {
            status: 200,
            body: body.to_owned(),
        })
    }

    const DRAFT: &str = "11111111-2222-4333-8444-555555555555";

    fn draft() -> NewIssue {
        NewIssue {
            id: DRAFT.to_owned(),
            team_id: "team-acm".to_owned(),
            title: "Fix the login".to_owned(),
            description: None,
            state_id: None,
            priority: Priority::None,
            assignee_id: None,
            label_ids: Vec::new(),
            project_id: None,
            cycle_id: None,
            estimate: None,
            due_date: None,
        }
    }

    fn found(id: &str) -> Result<Reply, Error> {
        said(&format!(
            r#"{{"data":{{"issue":{{"id":"{id}","identifier":"ACM-900","title":"Fix the login","priority":0,
            "updatedAt":"2026-11-09T14:20:00.000Z","url":"https://linear.app/x/900",
            "state":{{"id":"s","name":"Todo","type":"unstarted","position":0}},
            "labels":{{"nodes":[]}},"team":{{"id":"team-acm","key":"ACM"}},"cycle":null,"project":null,"assignee":null}}}}}}"#
        ))
    }

    const MADE: &str = r#"{"data":{"issueCreate":{"success":true,"issue":{"id":"issue-900","identifier":"ACM-900"}}}}"#;

    #[test]
    fn a_create_that_is_answered_is_sent_once() {
        let (session, sent) = answered(vec![said(MADE)]);
        let made = session.create(&draft()).unwrap();
        assert_eq!(
            made,
            Created {
                id: "issue-900".to_owned(),
                identifier: "ACM-900".to_owned(),
                url: String::new(),
            }
        );
        assert_eq!(sent.lock().unwrap().len(), 1);
    }

    /// No answer came: the program does not send again, it asks whether the issue is there, by the
    /// id of the draft, and it is.
    #[test]
    fn a_create_with_no_answer_asks_for_the_issue_by_the_draft_id_and_finds_it() {
        let (session, sent) = answered(vec![
            Err(Error::Unreachable("down".to_owned())),
            found(DRAFT),
        ]);
        let made = session.create(&draft()).unwrap();
        assert_eq!(made.id, DRAFT);
        assert_eq!(made.identifier, "ACM-900");
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].contains(r#""operationName":"IssueCreate""#));
        assert!(sent[1].contains(r#""operationName":"Issue""#));
        assert!(sent[1].contains(&format!(r#""variables":{{"id":"{DRAFT}"}}"#)));
        assert_eq!(
            sent.iter().filter(|b| b.contains("IssueCreate")).count(),
            1,
            "never sent twice"
        );
    }

    #[test]
    fn a_create_that_failed_and_has_no_issue_is_the_failure_it_was() {
        let (session, sent) = answered(vec![
            said(r#"{"errors":[{"message":"title is too long"}]}"#),
            said(r#"{"errors":[{"message":"Entity not found: Issue"}]}"#),
        ]);
        let failed = session.create(&draft()).unwrap_err();
        assert_eq!(
            failed,
            Error::Failed("Linear said: title is too long".to_owned())
        );
        assert_eq!(sent.lock().unwrap().len(), 2);
    }

    /// `issue(id:)` takes an identifier too, and a copy of another issue is not ours.
    #[test]
    fn an_issue_found_with_another_id_is_not_the_draft() {
        let (session, _) = answered(vec![
            Err(Error::Unreachable("down".to_owned())),
            found("another-issue"),
        ]);
        assert_eq!(
            session.create(&draft()).unwrap_err(),
            Error::Unreachable("down".to_owned())
        );
    }

    #[test]
    fn a_create_that_cannot_be_asked_about_is_not_known_to_have_failed() {
        let (session, sent) = answered(vec![
            Err(Error::Unreachable("down".to_owned())),
            Err(Error::Unreachable("down".to_owned())),
        ]);
        assert_eq!(session.create(&draft()).unwrap_err(), Error::Unsure);
        assert_eq!(sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn a_key_linear_does_not_take_is_not_asked_about() {
        for refusal in [
            Ok(Reply {
                status: 401,
                body: String::new(),
            }),
            Ok(Reply {
                status: 403,
                body: String::new(),
            }),
        ] {
            let (session, sent) = answered(vec![refusal]);
            assert!(session.create(&draft()).is_err());
            assert_eq!(sent.lock().unwrap().len(), 1);
        }
    }

    const TEAMS: &str =
        r#"{"data":{"teams":{"nodes":[{"id":"team-acm","key":"ACM","name":"Acme"}]}}}"#;
    const OPTIONS: &str = r#"{"data":{"team":{"id":"team-acm","defaultIssueState":null,"states":{"nodes":[]},
        "labels":{"nodes":[]},"projects":{"nodes":[]},"members":{"nodes":[]}},"issueLabels":{"nodes":[]}}}"#;

    #[test]
    fn the_choices_and_the_issues_opened_are_kept_until_the_lists_are_asked_for_again() {
        let (session, sent) = answered(vec![
            said(TEAMS),
            said(OPTIONS),
            found("issue-900"),
            said(OPTIONS),
            found("issue-900"),
        ]);
        assert_eq!(session.teams().unwrap().len(), 1);
        assert_eq!(session.teams().unwrap().len(), 1);
        session.options("team-acm").unwrap();
        session.options("team-acm").unwrap();
        session.detail("issue-900").unwrap();
        session.detail("issue-900").unwrap();
        assert_eq!(sent.lock().unwrap().len(), 3, "each asked for once");
        session.state().forget_kept();
        session.options("team-acm").unwrap();
        session.detail("issue-900").unwrap();
        assert_eq!(sent.lock().unwrap().len(), 5);
    }

    /// An issue as Linear says it, in a state of `kind`, given to the person (`me`) or to Deniz.
    fn wire(id: &str, title: &str, kind: &str, me: bool) -> String {
        let (who, is_me) = if me {
            ("u-ada", true)
        } else {
            ("u-deniz", false)
        };
        format!(
            r##"{{"id":"{id}","identifier":"ACM-{id}","title":"{title}","priority":2,
            "updatedAt":"2026-11-09T14:20:00.000Z","url":"https://linear.app/x/{id}",
            "state":{{"id":"s-{kind}","name":"State {kind}","type":"{kind}","position":1}},
            "labels":{{"nodes":[{{"id":"l-bug","name":"bug","color":"#eb5757"}}]}},
            "team":{{"id":"team-acm","key":"ACM"}},"cycle":null,"project":null,
            "assignee":{{"id":"{who}","displayName":"Someone","isMe":{is_me}}}}}"##
        )
    }

    fn issue_of(wire: &str) -> Issue {
        serde_json::from_str(wire).unwrap()
    }

    fn batch_reply(issues: &[String]) -> Result<Reply, Error> {
        said(&format!(
            r#"{{"data":{{"issueBatchUpdate":{{"success":true,"issues":[{}]}}}}}}"#,
            issues.join(",")
        ))
    }

    /// A session holding two issues in both lists (`a` and `b`, started, the person's), and what it
    /// sends.
    fn holding(replies: Vec<Result<Reply, Error>>) -> (Session, Arc<Mutex<Vec<String>>>) {
        let (session, sent) = answered(replies);
        let issues = vec![
            issue_of(&wire("a", "Add it", "started", true)),
            issue_of(&wire("b", "Fix it", "started", true)),
        ];
        let listing = Listing {
            issues,
            truncated: false,
        };
        session.shared.lock().lists = Some(Lists {
            viewer: Viewer {
                id: "u-ada".to_owned(),
                name: "Ada".to_owned(),
                organization: crate::linear::types::Organization {
                    name: "Acme".to_owned(),
                    url_key: "acme".to_owned(),
                },
            },
            mine: listing.clone(),
            shown: Some((Scope::All, listing)),
            filters: Filters::default(),
        });
        (session, sent)
    }

    fn ids_of(listing: &Listing) -> Vec<&str> {
        listing.issues.iter().map(|i| i.id.as_str()).collect()
    }

    /// The list held besides the person's own.
    fn shown(lists: &Lists) -> &Listing {
        &lists.shown.as_ref().expect("a list is shown").1
    }

    fn strings(of: &[&str]) -> Vec<String> {
        of.iter().map(|id| (*id).to_owned()).collect()
    }

    const COMMENT_ID: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";

    const POSTED: &str = r#"{"data":{"commentCreate":{"success":true,"comment":{"id":"c-1","body":"Looks right","createdAt":"2026-11-12T09:30:00.000Z","user":{"displayName":"Ada"}}}}}"#;

    /// An issue kept as the page opened it: its description and one comment.
    fn opened() -> Issue {
        let mut opened = issue_of(&wire("a", "Add it", "started", true));
        opened.description = Some("The words".to_owned());
        opened.comments = Some(
            serde_json::from_str(
                r#"{"nodes":[{"body":"First","createdAt":"2026-11-10T09:30:00.000Z","user":null}],
                "pageInfo":{"hasNextPage":false,"endCursor":null}}"#,
            )
            .unwrap(),
        );
        opened
    }

    #[test]
    fn a_comment_that_is_posted_is_among_the_comments_of_the_issue_kept_and_sent_once() {
        let (session, sent) = holding(vec![said(POSTED)]);
        // Kept by its id and by its identifier, as the page may have asked for it.
        for by in ["a", "ACM-a"] {
            session
                .shared
                .lock()
                .details
                .insert(by.to_owned(), opened());
        }
        let posted = session.comment(COMMENT_ID, "a", "Looks right").unwrap();
        assert_eq!(posted.body, "Looks right");
        for by in ["a", "ACM-a"] {
            let kept = session.detail(by).unwrap();
            let comments = kept.comments.unwrap().nodes;
            assert_eq!(comments.len(), 2, "{by}");
            assert_eq!(comments[1].body, "Looks right");
            assert_eq!(kept.description.as_deref(), Some("The words"));
        }
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "the kept issue is not asked for");
        assert!(sent[0].contains(r#""operationName":"CommentCreate""#));
        assert!(sent[0].contains(COMMENT_ID));
    }

    #[test]
    fn a_comment_with_no_answer_asks_for_the_issue_and_is_never_sent_twice() {
        let found = said(&format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            wire("a", "Add it", "started", true)
        ));
        let (session, sent) = holding(vec![Err(Error::Unreachable("timed out".to_owned())), found]);
        let failed = session.comment(COMMENT_ID, "a", "Looks right").unwrap_err();
        assert!(matches!(failed, Error::Unreachable(_)));
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].contains(r#""operationName":"CommentCreate""#));
        assert!(sent[1].contains(r#""operationName":"Issue""#));
    }

    #[test]
    fn a_comment_whose_answer_cannot_be_read_is_not_known_and_the_issue_is_asked_for() {
        for broken in [
            Ok(Reply {
                status: 502,
                body: "<html>Bad Gateway</html>".to_owned(),
            }),
            said("not json at all"),
        ] {
            let found = said(&format!(
                r#"{{"data":{{"issue":{}}}}}"#,
                wire("a", "Add it", "started", true)
            ));
            let (session, sent) = holding(vec![broken, found]);
            let failed = session.comment(COMMENT_ID, "a", "x").unwrap_err();
            assert_eq!(failed, Error::Unsure);
            assert_eq!(sent.lock().unwrap().len(), 2);
        }
    }

    #[test]
    fn a_comment_linear_says_no_to_or_a_key_it_does_not_take_asks_about_nothing() {
        for (reply, expected) in [
            (
                said(r#"{"data":{"commentCreate":{"success":false,"comment":null}}}"#),
                "failed",
            ),
            (
                said(r#"{"errors":[{"message":"Argument Validation Error"}]}"#),
                "failed",
            ),
            (
                Ok(Reply {
                    status: 403,
                    body: r#"{"errors":[{"message":"Forbidden"}]}"#.to_owned(),
                }),
                "forbidden",
            ),
            (
                Ok(Reply {
                    status: 401,
                    body: String::new(),
                }),
                "refused",
            ),
        ] {
            let (session, sent) = holding(vec![reply]);
            let failed = session.comment(COMMENT_ID, "a", "x").unwrap_err();
            let what = match &failed {
                Error::Failed(_) => "failed",
                Error::Forbidden => "forbidden",
                Error::Refused => "refused",
                other => panic!("not one a comment's answer says: {other:?}"),
            };
            assert_eq!(what, expected, "{failed:?}");
            assert_eq!(sent.lock().unwrap().len(), 1, "{failed:?}");
        }
    }

    /// The kind of the state the issue `id` is in, in `listing`.
    fn kind_in(listing: &Listing, id: &str) -> Option<StateKind> {
        listing
            .issues
            .iter()
            .find(|issue| issue.id == id)
            .map(|issue| issue.state.kind)
    }

    #[test]
    fn an_issue_that_is_done_stays_in_both_lists_in_its_new_state() {
        let (session, sent) = holding(vec![batch_reply(&[wire("a", "Add it", "completed", true)])]);
        let told = session
            .change(&strings(&["a"]), &Change::State("s-completed".to_owned()))
            .unwrap();
        assert_eq!(
            told,
            [Changed {
                id: "a".to_owned(),
                listed: true
            }]
        );
        let state = session.state();
        let lists = state.lists.as_ref().unwrap();
        assert_eq!(ids_of(&lists.mine), ["a", "b"]);
        assert_eq!(ids_of(shown(lists)), ["a", "b"]);
        assert_eq!(kind_in(&lists.mine, "a"), Some(StateKind::Completed));
        assert_eq!(kind_in(shown(lists), "a"), Some(StateKind::Completed));
        assert_eq!(sent.lock().unwrap().len(), 1, "no list is asked for again");
    }

    #[test]
    fn an_issue_given_to_another_leaves_my_issues_and_stays_in_all() {
        let (session, _) = holding(vec![
            batch_reply(&[wire("a", "Add it", "started", false)]),
            batch_reply(&[wire("b", "Fix it", "started", false)]),
        ]);
        let change = Change::Assignee(Some("u-deniz".to_owned()));
        let mine = session.change(&strings(&["a"]), &change).unwrap();
        assert!(!mine[0].listed, "the list in view is My issues");
        {
            let state = session.state();
            let lists = state.lists.as_ref().unwrap();
            assert_eq!(ids_of(&lists.mine), ["b"]);
            assert_eq!(ids_of(shown(lists)), ["a", "b"]);
            assert_eq!(
                shown(lists).issues[0]
                    .assignee
                    .as_ref()
                    .map(|a| a.id.as_str()),
                Some("u-deniz")
            );
        }
        session.state().view.scope = Scope::All;
        let all = session.change(&strings(&["b"]), &change).unwrap();
        assert!(all[0].listed, "All holds it still");
    }

    #[test]
    fn a_change_of_state_moves_the_issue_to_its_new_group_in_place() {
        let (session, _) = holding(vec![batch_reply(&[
            wire("a", "Add it", "backlog", true),
            wire("b", "Fix it", "backlog", true),
        ])]);
        let told = session
            .change(
                &strings(&["a", "b"]),
                &Change::State("s-backlog".to_owned()),
            )
            .unwrap();
        assert!(told.iter().all(|changed| changed.listed));
        let state = session.state();
        let lists = state.lists.as_ref().unwrap();
        assert_eq!(ids_of(&lists.mine), ["a", "b"]);
        assert!(
            lists
                .mine
                .issues
                .iter()
                .all(|issue| issue.state.kind == crate::linear::types::StateKind::Backlog)
        );
    }

    #[test]
    fn the_topic_is_woken_by_a_change_and_the_issue_kept_keeps_its_description() {
        let mut opened = issue_of(&wire("a", "Add it", "started", true));
        opened.description = Some("The words".to_owned());
        let (session, sent) = holding(vec![batch_reply(&[wire("a", "Added it", "started", true)])]);
        session
            .shared
            .lock()
            .details
            .insert("a".to_owned(), opened.clone());
        // It is kept by the identifier too, when the page asked for it by that.
        session
            .shared
            .lock()
            .details
            .insert("ACM-a".to_owned(), opened);
        let before = session.generation();
        session
            .change(&strings(&["a"]), &Change::Title("Added it".to_owned()))
            .unwrap();
        assert!(session.generation() > before);
        for by in ["a", "ACM-a"] {
            let kept = session.detail(by).unwrap();
            assert_eq!(kept.title, "Added it");
            assert_eq!(kept.description.as_deref(), Some("The words"));
        }
        assert_eq!(
            sent.lock().unwrap().len(),
            1,
            "the kept issue is not asked for"
        );
    }

    #[test]
    fn a_change_made_while_the_lists_are_on_their_way_has_them_asked_for_once_more() {
        let (session, _sent) =
            holding(vec![batch_reply(&[wire("a", "Added it", "started", true)])]);
        session.shared.lock().refreshing = true;
        session
            .change(&strings(&["a"]), &Change::Title("Added it".to_owned()))
            .unwrap();
        assert!(
            session.shared.lock().refresh_wanted,
            "the list on its way was asked for before the change, and would put the old title back"
        );
        // That list comes, from before the change: it is let go, and the lists keep the change.
        let old = {
            let state = session.shared.lock();
            let lists = state.lists.as_ref().unwrap();
            let mut old = lists.clone();
            old.mine.issues[0] = issue_of(&wire("a", "Add it", "started", true));
            old
        };
        session.shared.lock().landed(Ok(old));
        let state = session.state();
        assert!(!state.refreshing);
        assert_eq!(
            state.lists.as_ref().unwrap().mine.issues[0].title,
            "Added it",
            "the old title was not put back"
        );
        // Where none is on its way nothing more is asked for.
        let (session, _sent) =
            holding(vec![batch_reply(&[wire("a", "Added it", "started", true)])]);
        session
            .change(&strings(&["a"]), &Change::Title("Added it".to_owned()))
            .unwrap();
        assert!(!session.shared.lock().refresh_wanted);
    }

    #[test]
    fn from_one_to_fifty_issues_at_most_and_nothing_is_sent_for_more() {
        let (session, sent) = holding(Vec::new());
        let many: Vec<String> = (0..51).map(|n| format!("i{n}")).collect();
        for bad in [Vec::new(), many] {
            assert!(
                session
                    .change(&bad, &Change::Title("x".to_owned()))
                    .is_err()
            );
        }
        assert!(sent.lock().unwrap().is_empty());
    }

    /// No answer came: the change is not sent again, the issue is asked for, and what Linear says of
    /// it is what the lists hold; the failure is told.
    #[test]
    fn a_change_with_no_answer_asks_for_the_issues_and_is_never_sent_twice() {
        let found = said(&format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            wire("a", "Add it", "completed", true)
        ));
        let (session, sent) = holding(vec![Err(Error::Unreachable("timed out".to_owned())), found]);
        let failed = session
            .change(&strings(&["a"]), &Change::State("s-completed".to_owned()))
            .unwrap_err();
        assert!(matches!(failed, Error::Unreachable(_)));
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert!(sent[0].contains(r#""operationName":"IssueBatchUpdate""#));
        assert!(sent[1].contains(r#""operationName":"Issue""#));
        drop(sent);
        // Linear did make it: the list says so.
        let state = session.state();
        let mine = &state.lists.as_ref().unwrap().mine;
        assert_eq!(kind_in(mine, "a"), Some(StateKind::Completed));
    }

    #[test]
    fn asking_again_stops_where_linear_cannot_be_reached() {
        let (session, sent) = holding(vec![
            Err(Error::Unreachable("down".to_owned())),
            Err(Error::Unreachable("down".to_owned())),
        ]);
        let failed = session.change(&strings(&["a", "b"]), &Change::Title("x".to_owned()));
        assert!(matches!(failed, Err(Error::Unreachable(_))));
        assert_eq!(sent.lock().unwrap().len(), 2, "one change, one question");
    }

    /// A gateway's 502 or a body cut short is not Linear's no: the change may have been made, so the
    /// issues are asked for, and it is told that it is not known. Nothing is sent twice.
    #[test]
    fn a_change_whose_answer_cannot_be_read_asks_for_the_issues_and_is_not_known() {
        for broken in [
            Ok(Reply {
                status: 502,
                body: "<html>Bad Gateway</html>".to_owned(),
            }),
            said("not json at all"),
        ] {
            let found = said(&format!(
                r#"{{"data":{{"issue":{}}}}}"#,
                wire("a", "Add it", "completed", true)
            ));
            let (session, sent) = holding(vec![broken, found]);
            let failed = session
                .change(&strings(&["a"]), &Change::State("s-completed".to_owned()))
                .unwrap_err();
            assert_eq!(failed, Error::Unsure);
            let sent = sent.lock().unwrap();
            assert_eq!(sent.len(), 2);
            assert!(sent[0].contains(r#""operationName":"IssueBatchUpdate""#));
            assert!(sent[1].contains(r#""operationName":"Issue""#));
            drop(sent);
            // Linear did make it: the list says so.
            let state = session.state();
            let mine = &state.lists.as_ref().unwrap().mine;
            assert_eq!(kind_in(mine, "a"), Some(StateKind::Completed));
        }
    }

    /// What is asked for after a change that had no answer is a change of the lists as well: a list on
    /// its way would put the old values back.
    #[test]
    fn asking_for_the_issues_after_no_answer_lets_go_of_a_list_on_its_way() {
        let found = said(&format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            wire("a", "Add it", "completed", true)
        ));
        let (session, _sent) =
            holding(vec![Err(Error::Unreachable("timed out".to_owned())), found]);
        session.shared.lock().refreshing = true;
        let _ = session.change(&strings(&["a"]), &Change::State("s-completed".to_owned()));
        let state = session.shared.lock();
        assert!(state.refresh_wanted && state.stale);
    }

    #[test]
    fn a_change_linear_says_it_did_not_make_leaves_the_lists_as_they_are() {
        let still = said(&format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            wire("a", "Add it", "started", true)
        ));
        let (session, sent) = holding(vec![
            said(r#"{"errors":[{"message":"state does not belong to the team"}],"data":null}"#),
            still,
        ]);
        let failed = session
            .change(&strings(&["a"]), &Change::State("s-x".to_owned()))
            .unwrap_err();
        assert_eq!(
            failed,
            Error::Failed("Linear said: state does not belong to the team".to_owned())
        );
        assert_eq!(
            sent.lock().unwrap().len(),
            1,
            "Linear answered: nothing more is asked"
        );
        let state = session.state();
        assert_eq!(ids_of(&state.lists.as_ref().unwrap().mine), ["a", "b"]);
    }

    #[test]
    fn a_key_linear_does_not_take_has_not_changed_anything_and_is_not_asked_about() {
        for refusal in [401, 403] {
            let (session, sent) = holding(vec![Ok(Reply {
                status: refusal,
                body: String::new(),
            })]);
            assert!(
                session
                    .change(&strings(&["a"]), &Change::Title("x".to_owned()))
                    .is_err()
            );
            assert_eq!(sent.lock().unwrap().len(), 1);
        }
    }

    #[test]
    fn an_issue_the_answer_leaves_out_is_asked_for() {
        let found = said(&format!(
            r#"{{"data":{{"issue":{}}}}}"#,
            wire("b", "Fix it", "completed", true)
        ));
        let (session, sent) = holding(vec![
            batch_reply(&[wire("a", "Add it", "completed", true)]),
            found,
        ]);
        let told = session
            .change(
                &strings(&["a", "b"]),
                &Change::State("s-completed".to_owned()),
            )
            .unwrap();
        assert_eq!(told.len(), 2);
        assert_eq!(sent.lock().unwrap().len(), 2);
        let state = session.state();
        let mine = &state.lists.as_ref().unwrap().mine;
        assert_eq!(kind_in(mine, "a"), Some(StateKind::Completed));
        assert_eq!(kind_in(mine, "b"), Some(StateKind::Completed));
    }

    #[test]
    fn a_view_call_told_late_does_not_undo_a_later_one_of_the_same_member() {
        let session = session();
        session.set_view(Some(2), None, None, Some("rate".to_owned()), None);
        session.set_view(Some(1), None, None, Some("rat".to_owned()), None);
        assert_eq!(session.state().view.filter, "rate");
        session.set_view(Some(3), None, None, Some("rates".to_owned()), None);
        assert_eq!(session.state().view.filter, "rates");
    }

    #[test]
    fn a_call_is_held_to_the_order_only_of_the_members_it_sets() {
        let session = session();
        // The filter call has the higher number and comes first: the scope call is not its.
        session.set_view(Some(5), None, None, Some("bug".to_owned()), None);
        session.set_view(Some(4), Some(Scope::All), None, None, None);
        let folded = HashMap::from([("todo".to_owned(), true)]);
        session.set_view(Some(3), None, None, None, Some(folded.clone()));
        let state = session.state();
        assert_eq!(state.view.scope, Scope::All);
        assert_eq!(state.view.filter, "bug");
        assert_eq!(state.view.closed, folded);
    }

    #[test]
    fn a_page_that_begins_again_is_heard_from_its_first_call() {
        let session = session();
        session.set_view(Some(9), None, None, Some("old".to_owned()), None);
        session.forget_view_order();
        session.set_view(Some(1), None, None, Some("new".to_owned()), None);
        assert_eq!(session.state().view.filter, "new");
        // A call with no number is the latest.
        session.set_view(None, None, None, Some("last".to_owned()), None);
        assert_eq!(session.state().view.filter, "last");
    }

    fn team(id: &str) -> Scope {
        Scope::Team(id.to_owned())
    }

    #[test]
    fn a_page_that_goes_to_another_list_has_it_asked_for_and_lets_go_of_the_one_it_left() {
        let (session, _) = holding(vec![]);
        session.shared.lock().refresh_wanted = false;
        session.set_view(None, Some(team("t-acm")), None, None, None);
        {
            let state = session.state();
            assert_eq!(state.view.scope, team("t-acm"));
            assert!(state.refresh_wanted_now(), "the list is asked for");
            let lists = state.lists.as_ref().unwrap();
            assert!(
                lists.shown.is_none(),
                "All issues, which it left, is let go of"
            );
            assert!(lists.of(&team("t-acm"), &Filters::default()).is_none());
            assert_eq!(
                ids_of(lists.of(&Scope::Mine, &Filters::default()).unwrap()),
                ["a", "b"]
            );
        }
        // The same list again asks for nothing more; My issues needs nothing asked for.
        session.shared.lock().refresh_wanted = false;
        session.set_view(None, Some(team("t-acm")), None, None, None);
        assert!(!session.state().refresh_wanted_now());
        session.set_view(None, Some(Scope::Mine), None, None, None);
        assert!(!session.state().refresh_wanted_now());
    }

    #[test]
    fn names_that_did_not_come_are_asked_for_again_at_r_and_not_with_each_list() {
        let (session, _) = holding(vec![]);
        {
            let mut state = session.state();
            state.names = None;
            state.names_error = None;
            state.everything = false;
            assert!(state.asks_for_everything(false), "never asked for");
            state.named(Err(Error::Failed("a favorite is gone".to_owned())));
            assert!(
                !state.asks_for_everything(false),
                "asked for, and did not come: not with the next list"
            );
            assert!(state.asks_for_everything(true), "the five minutes");
            state.everything = true;
            assert!(state.asks_for_everything(false), "`r`");
        }
    }

    #[test]
    fn going_back_to_my_issues_from_a_list_that_failed_asks_again_and_does_not_keep_the_error() {
        let (session, _) = holding(vec![]);
        session.set_view(None, Some(team("t-gone")), None, None, None);
        session.shared.lock().refresh_wanted = false;
        session
            .state()
            .landed(Err(Error::Failed("nothing there".to_owned())));
        assert!(session.state().error.is_some());
        session.set_view(None, Some(Scope::Mine), None, None, None);
        assert!(
            session.state().refresh_wanted_now(),
            "My issues is asked for again, so that the error of the list left is let go of"
        );
    }

    #[test]
    fn a_list_that_comes_after_the_page_went_elsewhere_is_let_go_of_and_the_new_one_asked_for() {
        let (session, _) = holding(vec![]);
        let mut lists = session.state().lists.clone().unwrap();
        lists.shown = Some((team("t-acm"), lists.mine.clone()));
        {
            let mut state = session.state();
            state.view.scope = Scope::View("v-1".to_owned());
            state.refresh_wanted = false;
            state.landed(Ok(lists.clone()));
            assert!(state.refresh_wanted_now());
            let held = state.lists.as_ref().unwrap();
            assert!(held.shown.is_none());
            assert_eq!(ids_of(&held.mine), ["a", "b"], "My issues is as it came");
            // The list it looks at is the one that came.
            state.view.scope = team("t-acm");
            state.refresh_wanted = false;
            state.landed(Ok(lists));
            assert!(!state.refresh_wanted_now());
            assert!(state.lists.as_ref().unwrap().shown.is_some());
        }
    }

    fn labelled(name: &str) -> Filters {
        Filters::new(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![name.to_owned()],
            Vec::new(),
        )
        .expect("a filter")
    }

    #[test]
    fn filters_narrow_the_list_even_where_it_is_my_issues_and_have_it_asked_for_with_them() {
        let (session, _) = holding(vec![]);
        session.shared.lock().refresh_wanted = false;
        // Nothing narrows My issues: it is what is held, and nothing is asked for.
        session.set_view(None, None, Some(Filters::default()), None, None);
        assert!(!session.state().refresh_wanted_now());
        session.set_view(None, None, Some(labelled("bug")), None, None);
        {
            let state = session.state();
            assert_eq!(state.view.filters, labelled("bug"));
            assert!(state.refresh_wanted_now(), "the narrowed list is asked for");
            let lists = state.lists.as_ref().unwrap();
            // What is held is My issues whole: it is not the list narrowed, and is not shown as it.
            assert!(lists.of(&Scope::Mine, &labelled("bug")).is_none());
            assert!(lists.of(&Scope::Mine, &Filters::default()).is_some());
        }
        // The same filters again ask for nothing more; letting go of them asks for nothing either
        // (My issues is held), and what was held narrowed is let go of.
        session.shared.lock().refresh_wanted = false;
        session.set_view(None, None, Some(labelled("bug")), None, None);
        assert!(!session.state().refresh_wanted_now());
        session.set_view(None, None, Some(Filters::default()), None, None);
        let state = session.state();
        assert!(!state.refresh_wanted_now());
        assert!(state.lists.as_ref().unwrap().shown.is_none());
    }

    #[test]
    fn a_list_that_comes_narrowed_by_other_filters_than_the_page_has_now_is_not_shown() {
        let (session, _) = holding(vec![]);
        let mut lists = session.state().lists.clone().unwrap();
        lists.shown = Some((Scope::Mine, lists.mine.clone()));
        lists.filters = labelled("api");
        {
            let mut state = session.state();
            state.view.filters = labelled("bug");
            state.refresh_wanted = false;
            state.landed(Ok(lists.clone()));
            let held = state.lists.as_ref().unwrap();
            // It was asked for with other filters: it is let go of, and the page's are asked for.
            assert!(held.of(&Scope::Mine, &labelled("bug")).is_none());
            assert!(held.shown.is_none());
            assert!(state.refresh_wanted_now());
        }
    }

    #[test]
    fn a_search_keeps_the_issues_it_holds_that_are_done_and_a_narrowed_my_issues_those_that_are_still_mine()
     {
        let (session, _) = holding(vec![]);
        let held = issue_of(&wire("a", "Add it", "completed", true));
        {
            let mut state = session.state();
            let lists = state.lists.as_mut().unwrap();
            lists.shown = Some((
                Scope::Search("add".to_owned()),
                Listing {
                    issues: vec![held],
                    truncated: false,
                },
            ));
            state.view.scope = Scope::Search("add".to_owned());
            // (One that was done is not let go of.)
            assert!(state.put(&issue_of(&wire("a", "Renamed", "completed", true))));
            let lists = state.lists.as_ref().unwrap();
            assert_eq!(ids_of(shown(lists)), ["a"]);
        }
        {
            let mut state = session.state();
            let lists = state.lists.as_mut().unwrap();
            lists.filters = labelled("bug");
            lists.shown = Some((Scope::Mine, lists.mine.clone()));
            state.view.scope = Scope::Mine;
            state.view.filters = labelled("bug");
            // Given to another it is not the person's, and goes from their list, narrowed or not.
            assert!(!state.put(&issue_of(&wire("a", "Given", "started", false))));
            let lists = state.lists.as_ref().unwrap();
            assert_eq!(ids_of(shown(lists)), ["b"]);
        }
    }

    #[test]
    fn an_issue_changed_is_kept_as_it_is_in_the_list_that_holds_it_and_not_added_to_another() {
        let (session, _) = holding(vec![]);
        let held = issue_of(&wire("a", "Add it", "started", true));
        {
            let mut state = session.state();
            let lists = state.lists.as_mut().unwrap();
            lists.shown = Some((
                team("t-acm"),
                Listing {
                    issues: vec![held],
                    truncated: false,
                },
            ));
            state.view.scope = team("t-acm");
            // One that the list holds is as it is now; one that it does not hold is not put in it
            // (Linear's own rule for the list is not known here), and does not say it is listed.
            assert!(state.put(&issue_of(&wire("a", "Renamed", "started", true))));
            assert!(!state.put(&issue_of(&wire("c", "Other", "started", true))));
            let lists = state.lists.as_ref().unwrap();
            assert_eq!(ids_of(shown(lists)), ["a"]);
            assert_eq!(shown(lists).issues[0].title, "Renamed");
            // One that is done stays in it, in its new state.
            assert!(state.put(&issue_of(&wire("a", "Renamed", "completed", true))));
            let shown = shown(state.lists.as_ref().unwrap());
            assert_eq!(kind_in(shown, "a"), Some(StateKind::Completed));
        }
    }
}
