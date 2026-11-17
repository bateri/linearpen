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
  show      one issue: its fields, description, comments and ties to other issues
  create    make an issue in a team
  update    change the status, priority, assignee, labels, project, cycle, estimate, due date
            or title of one issue or of several
  comment   write a comment under an issue
  teams     the teams, or the names that a team's issues can be given
  help      linearpen help COMMAND, or linearpen COMMAND --help, tells the options of a command

A name is written as Linear shows it; capital letters do not matter. Where a name is none of the
team's, or is more than one, nothing is sent and the names there are come back instead.

In a terminal that shows pencere views (bateri) linearpen opens in the pane. In any other it
prints the issues as text and exits. The Linear API key is read from LINEAR_API_KEY, or from the
file `key` in ~/.config/linearpen.

linearpen writes to Linear only what you ask of it: in the pane, an issue you make with its New
issue form, a change you make to an issue (its status, priority, assignee, labels, project or
title), a comment, and the status of an issue an agent is started on, where the form says In
Progress; on the command line, the same three with create, update and comment. It never writes a
description of an issue that exists, and never moves an issue to another team. `linearpen --text`,
list, show and teams only read.

Each command ends with 0 when it is done. It ends with 1 where Linear could not do it, said no, or
has nothing by a name given, with 2 where the command line is not understood or there is no usable
key, and with 3 where a write was sent and it is not known whether Linear made it: look in Linear
before running it again.
";

pub const LIST: &str = "\
linearpen list: the issues of a list, as text

usage: linearpen list [LIST] [NARROWING] [--json]

LIST is one of these (the open issues of My issues, if none is given):
  --all             all issues of the workspace
  --team KEY        the issues of a team, by key or by name
  --cycle current   with --team: the issues of the team's cycle that is running, or of the cycle
  --cycle N         with this number (the running one and the ones that come next)
  --view NAME       a saved view of issues
  --search WORDS    what a search for the words finds, closed issues too

NARROWING gives the issues that are all of it; the values of one option are any of them (an option
may be repeated):
  --status S        the name of a status
  --priority P      urgent, high, medium, low or none
  --assignee A      me, none, or the name of a person
  --label L         the name of a label
  --project P       none, or the name of a project

  --json            print JSON instead of text; its fields are in
                    https://github.com/bateri/linearpen/blob/main/docs/cli.md

The open issues are listed; to find a closed one, search for it.
";

pub const SHOW: &str = "\
linearpen show: one issue

usage: linearpen show KEY-N [--json]

Prints its title, status, priority, assignee, labels, team, project, cycle, estimate, due date,
link and branch name, its description, the issues it is a part of, made of, or tied to, and its
comments. With --json the same, as JSON; its fields are in
https://github.com/bateri/linearpen/blob/main/docs/cli.md
";

pub const CREATE: &str = "\
linearpen create: make an issue

usage: linearpen create --team KEY --title TITLE [OPTIONS] [--json]

  --team KEY          the team, by key or by name (linearpen teams lists them)
  --title TITLE       one line
  --description TEXT  the words of the description, or - for the words on stdin
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
takes. The issue is made once; if the answer is lost, linearpen asks Linear for the issue by the id
it made for it, and ends with 3 where that does not settle it.
";

pub const UPDATE: &str = "\
linearpen update: change issues

usage: linearpen update KEY-N [KEY-N] [OPTIONS] [--json]

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
  --json            print JSON instead of text

Each option is given once but --label and --unlabel. linearpen never changes an issue's description
or team. Prints each issue as Linear says it is now.
";

pub const COMMENT: &str = "\
linearpen comment: write a comment under an issue

usage: linearpen comment KEY-N --body TEXT [--json]

  --body TEXT   the words of the comment, or - for the words on stdin
  --json        print JSON instead of text

The comment is posted once, in the words given. Prints that it was added.
";

pub const TEAMS: &str = "\
linearpen teams: the teams, and what their issues can be given

usage: linearpen teams [KEY] [--json]

Without a key: the teams, by key and name. With one: the statuses, labels, people, projects and
cycles of that team and the sizes it estimates by, as they are written in the options of create,
update and list. With --json the same, as JSON; its fields are in
https://github.com/bateri/linearpen/blob/main/docs/cli.md
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
        _ => return None,
    })
}
