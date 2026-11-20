//! What `linearpen --help`, `linearpen help COMMAND` and `linearpen COMMAND --help` say.

pub const USAGE: &str = "\
linearpen: your Linear issues in the terminal pane, and on the command line

usage: linearpen [--text]
       linearpen COMMAND [OPTIONS]

  --text           print your issues as text and exit, also in a terminal that shows views
  -V, --version    print the version
  -h, --help       print this

commands:
  list      the issues of a list, narrowed by status, priority, assignee, label or project
  show      one issue: its fields, description, comments, files and ties to other issues;
            the files can be saved
  create    make an issue in a team, with files if you give them
  update    change the status, priority, assignee, labels, project, cycle, estimate, due date
            or title of one issue or of several, or write the description of one, or add files
            to the end of it
  comment   write a comment under an issue, with files if you give them, or edit one of yours
  teams     the teams, or the names that a team's issues can be given
  agent     go to the pane of the agent at work on an issue (in bateri)
  help      linearpen help COMMAND, or linearpen COMMAND --help, tells the options of a command

A name is written as Linear shows it; capital letters do not matter. Where a name is none of the
team's, or is more than one, nothing is sent and the names there are come back instead. An issue is
written like ACM-412, or as Linear's id of it (a UUID).

In a terminal that shows pencere views (bateri) linearpen opens in the pane. In any other it
prints the issues as text and exits. The Linear API key is read from LINEAR_API_KEY, or from the
file `key` in ~/.config/linearpen.

linearpen writes to Linear only what you ask of it: in the pane, an issue you make with its New
issue form, a change you make to an issue (its status, priority, assignee, labels, project or
title), a description you edit, a comment, a comment of yours you edit, the files you add to a
comment, to a new issue or to the end of a description, and the status of an issue an agent is
started on, where the form says In Progress; on the command line, the same with create, update and
comment. It never moves an issue to another team, and never deletes or archives anything.
`linearpen --text`, list, show and teams only read, and agent writes nothing.

Files are of any kind, up to 100 MiB each and ten at a time (--attach PATH). They are put in
Linear's storage and named at the end of the words: a picture (PNG, JPEG, GIF, WebP) as a picture
and any other file as a link. A file in an issue is saved with `linearpen show KEY-N --files DIR`;
only files that are on Linear are fetched, and the key goes to Linear's own hosts and nowhere else.

Each command ends with 0 when it is done. It ends with 1 where Linear could not do it, said no, or
has nothing by a name given (for agent, where no agent is at work on the issue), with 2 where the command line is not understood, a file given with
--attach cannot be sent, or there is no usable key, and with 3 where a write was sent and it is not
known whether Linear made it: look in Linear before running it again.
";

pub const LIST: &str = "\
linearpen list: the issues of a list, as text

usage: linearpen list [LIST] [NARROWING] [--json]

LIST is one of these (My issues, if none is given):
  --all             all issues of the workspace
  --team KEY        the issues of a team, by key or by name
  --cycle current   with --team: the issues of the team's cycle that is running, or of the cycle
  --cycle N         with this number (the running one and the ones that come next)
  --view NAME       a saved view of issues
  --search WORDS    what a search for the words finds

NARROWING gives the issues that are all of it; the values of one option are any of them (an option
may be repeated):
  --status S        the name of a status
  --priority P      urgent, high, medium, low or none
  --assignee A      me, none, or the name of a person
  --label L         the name of a label
  --project P       none, or the name of a project

  --json            print JSON instead of text; its fields are in
                    https://github.com/bateri/linearpen/blob/main/docs/cli.md

The open issues are listed, up to 500. --status lists the done and canceled ones too (--status
Done), the ones that changed last first; a search finds any. In a bateri pane, an issue an agent is
at work on ends its line with the agent and its pane, `Claude at work in p41`, or `Claude idle in
p41` where the pane has written nothing for ten seconds (the agent is done, or waits for you):
linearpen agent KEY-N goes there.
";

pub const SHOW: &str = "\
linearpen show: one issue

usage: linearpen show KEY-N [--files DIR] [--json]

  --files DIR   save the files of the issue that are on Linear in DIR (made, with mode 0700, if
                it is not there) and print the path of each
  --json        print JSON instead of text

Prints its title, status, priority, assignee, labels, team, project, cycle, estimate, due date,
link and branch name, in a bateri pane the agent at work on it (as list says it), its description,
the issues it is a part of, made of, or tied to, and its comments. Each picture in the description or a comment is written where it stands as [image N: name],
and each other file of Linear's as [file N: name]; a list of them ends the text: the number, where it
is, and its address, or its file where --files was given. A file is of any kind and at most 100 MiB.
It is named N-name.ext, the extension from what the file holds (png, jpg, gif, webp, pdf), else the
one its name has; a file of that name with the same bytes is used again, one with other bytes gets
-2, -3 after the name, and none is written over. A file that cannot be had says why in its line (not
on Linear, too large, could not be fetched, too many files, too much in all) and the command still ends
with 0. A run saves at most 100 files and 1 GiB. With --json the same, as JSON; its fields are in
https://github.com/bateri/linearpen/blob/main/docs/cli.md
";

pub const CREATE: &str = "\
linearpen create: make an issue

usage: linearpen create --team KEY --title TITLE [OPTIONS] [--json]

  --team KEY          the team, by key or by name (linearpen teams lists them)
  --title TITLE       one line
  --description TEXT  the words of the description, or - for the words on stdin
  --attach PATH       a file, up to 100 MiB, put at the end of the description; it may be
                      repeated, up to ten times
  --status S          the name of a status of the team (the team's own if none is given)
  --priority P        urgent, high, medium, low or none
  --assignee A        me, or the name of a person of the team
  --label L           the name of a label; it may be repeated
  --project P         the name of a project
  --cycle C           current, or the number of a cycle
  --estimate E        a size of the team's scale, or a number of points
  --due DAY           a day, written 2026-11-20
  --json              print JSON instead of text

Prints the identifier and the link of the issue made. linearpen teams KEY lists the names each option
takes. The files of --attach are looked at before Linear is asked anything (status 2), and
are put in Linear's storage before the issue is made: if one cannot be, no issue is made.
The issue is made once; if the answer is lost, linearpen asks Linear for the issue by the id
it made for it, and ends with 3 where that does not settle it.
";

pub const UPDATE: &str = "\
linearpen update: change issues

usage: linearpen update KEY-N [KEY-N] [OPTIONS] [--json]
       linearpen update KEY-N --description TEXT [--attach PATH] [--json]
       linearpen update KEY-N --attach PATH [--attach PATH] [--json]

All the options given are one change: it is made in every issue given or in none. The issues are
of one team. At most 50 issues are changed at once.

  --status S        the name of a status of the team
  --priority P      urgent, high, medium, low or none
  --assignee A      me, none, or the name of a person of the team
  --label L         the name of a label the issues get; it may be repeated
  --unlabel L       the name of a label the issues lose; it may be repeated
  --project P       none, or the name of a project
  --cycle C         none, current, or the number of a cycle
  --estimate E      none, a size of the team's scale, or a number of points
  --due DAY         none, or a day, written 2026-11-20
  --title TITLE     one line, with one issue only
  --description TEXT  the whole description of one issue, in the place of the one it has, or -
                    for the words on stdin; with --attach only (its files go at the end)
  --attach PATH     a file, up to 100 MiB, put at the end of the description of one issue,
                    after a blank line; it may be repeated, up to ten times; it is the only
                    option of its update but --description
  --json            print JSON instead of text

Each option is given once but --label, --unlabel and --attach. --description writes the words given
over the description Linear holds: read it first (linearpen show KEY-N --json) to keep what is in it.
Without it, --attach adds files after the description Linear holds (it is read whole first, and
nothing is written if it cannot be). linearpen never changes an issue's team. The files of --attach
are looked at before Linear is asked anything (status 2); if one cannot be put in Linear's
storage nothing is written. Prints each issue as Linear says it is now.
";

pub const COMMENT: &str = "\
linearpen comment: write a comment under an issue

usage: linearpen comment KEY-N [--body TEXT] [--attach PATH] [--json]
       linearpen comment KEY-N --edit last|ID --body TEXT [--json]

  --body TEXT    the words of the comment, or - for the words on stdin
  --attach PATH  a file, up to 100 MiB, put at the end of the comment; it may be repeated, up to
                 ten times
  --edit C       edit a comment of yours instead of writing one: last, the last you wrote on the
                 issue, or the id of one (linearpen show KEY-N --json); --body is its words, whole
  --json         print JSON instead of text

One of --body and --attach is needed. The files of --attach are looked at before Linear is asked
anything (status 2), and are put in Linear's storage first: if one cannot be, no comment is posted.
The comment is posted once, in the words given with the files after them. Prints that it was added,
and with how many files. With --edit the words given take the place of the comment's, which must be
yours (Linear lets only the one who wrote a comment edit it), and no file goes with them.
";

pub const TEAMS: &str = "\
linearpen teams: the teams, and what their issues can be given

usage: linearpen teams [KEY] [--json]

Without a key: the teams, by key and name. With one: the statuses, labels, people, projects and
cycles of that team and the sizes it estimates by, as they are written in the options of create,
update and list. With --json the same, as JSON; its fields are in
https://github.com/bateri/linearpen/blob/main/docs/cli.md
";

pub const AGENT: &str = "\
linearpen agent: go to the agent at work on an issue

usage: linearpen agent KEY-N

In a bateri pane, takes the keyboard to the pane of the agent (Claude, Codex or Cursor) at work on the
issue, as the mark on its line in the pane does. An agent is at work on an issue where its pane, or
the tab it is in, has the issue's identifier for a name, which is what Work on gives it; of two, the
one started last. Prints where it went, and ends with 1 where no agent is at work on the issue,
linearpen is not in a bateri pane, or bateri does not go there. list and show say which issues an
agent is at work on, and in which pane.
";

/// The words of a command, `linearpen help COMMAND`.
#[must_use]
pub fn of(command: &str) -> Option<&'static str> {
    Some(match command {
        "list" => LIST,
        "show" => SHOW,
        "create" => CREATE,
        "update" => UPDATE,
        "comment" => COMMENT,
        "teams" => TEAMS,
        "agent" => AGENT,
        _ => return None,
    })
}
