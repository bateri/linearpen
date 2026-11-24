//! What is asked of Linear: every operation of the program, as constant text. The pen reads, and
//! writes five things: the issue the person makes, the change the person makes to issues (the words of
//! a description among them, as the person edits them), the comment the person writes, the edit of a
//! comment of theirs, and the notifications of an issue the person marks read in their inbox; and for
//! the pictures the person adds it asks Linear where to put one (`FileUpload`). Every operation is a
//! plain `query` except `IssueCreate`, `IssueBatchUpdate`, `CommentCreate`, `CommentUpdate`,
//! `FileUpload` and `NotificationMarkReadAll`, and `ALLOWED_MUTATIONS`
//! is the one list of what may be a `mutation`: the client refuses to send any other (`mod.rs`), and
//! the tests scan `ALL` for one. Nothing from outside is put into the text: what varies goes in the
//! `variables` of the request.

use serde::Serialize;

/// The mutations the program may send, by name: the one list of what it may write. Anything else
/// that is a `mutation` is refused before it is sent, however it is named.
pub const ALLOWED_MUTATIONS: [&str; 6] = [
    "IssueCreate",
    "IssueBatchUpdate",
    "CommentCreate",
    "CommentUpdate",
    "FileUpload",
    "NotificationMarkReadAll",
];

/// One operation: its name (which the stand-in `curl` of the tests tells them apart by) and its text.
#[derive(Debug, Clone, Copy)]
pub struct Operation {
    pub name: &'static str,
    pub text: &'static str,
}

impl Operation {
    /// Whether the text is one plain `query` and nothing that writes.
    #[must_use]
    pub fn reads_only(&self) -> bool {
        let lower = self.text.to_ascii_lowercase();
        lower.trim_start().starts_with("query ")
            && !lower.contains("mutation")
            && !lower.contains("subscription")
    }

    /// Whether the text is one `mutation` of the list, named as the text names it: it is the text
    /// that is read, not the `name`, and there is no second operation behind it.
    #[must_use]
    pub fn is_listed_write(&self) -> bool {
        let lower = self.text.to_ascii_lowercase();
        let Some(rest) = self.text.strip_prefix("mutation ") else {
            return false;
        };
        let named: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        ALLOWED_MUTATIONS.contains(&named.as_str())
            && named == self.name
            && lower.matches("mutation").count() == 1
            && !lower.contains("subscription")
            && !lower.contains("query ")
    }

    /// Whether the client may send it: a plain query, or the one mutation that is listed.
    #[must_use]
    pub fn allowed(&self) -> bool {
        self.reads_only() || self.is_listed_write()
    }
}

/// The issue fields of a row of the list, with the ids of what a change names (the team, the state,
/// the labels, the project, the cycle, the person). The description, the comments and the issues
/// around wait for the issue to be opened.
macro_rules! issue_fields {
    () => {
        "id identifier title priority updatedAt url branchName \
         state { id name type position } \
         labels(first: 10) { nodes { id name color } } \
         team { id key } cycle { id number name } project { id name } estimate dueDate \
         assignee { id displayName isMe }"
    };
}

macro_rules! viewer_fields {
    () => {
        "id name organization { name urlKey }"
    };
}

/// Whose key it is: the cheap question that says whether a key is one.
pub const VERIFY: Operation = Operation {
    name: "Verify",
    text: concat!("query Verify { viewer { ", viewer_fields!(), " } }"),
};

/// The issues assigned to the person.
pub const MY_ISSUES: Operation = Operation {
    name: "MyIssues",
    text: concat!(
        "query MyIssues($first: Int!, $after: String, $filter: IssueFilter) { viewer { ",
        viewer_fields!(),
        " assignedIssues(first: $first, after: $after, orderBy: updatedAt, filter: $filter) { \
         nodes { ",
        issue_fields!(),
        " } pageInfo { hasNextPage endCursor } } } }"
    ),
};

/// The issues of the whole workspace.
pub const ALL_ISSUES: Operation = Operation {
    name: "AllIssues",
    text: concat!(
        "query AllIssues($first: Int!, $after: String, $filter: IssueFilter) { viewer { ",
        viewer_fields!(),
        " } issues(first: $first, after: $after, orderBy: updatedAt, filter: $filter) { \
         nodes { ",
        issue_fields!(),
        " } pageInfo { hasNextPage endCursor } } }"
    ),
};

/// What a search for some words finds: issues of the whole workspace, as Linear ranks them. The words are a variable, like everything the person typed.
pub const SEARCH_ISSUES: Operation = Operation {
    name: "SearchIssues",
    text: concat!(
        "query SearchIssues($term: String!, $first: Int!, $after: String, $filter: IssueFilter) { viewer { ",
        viewer_fields!(),
        " } searchIssues(term: $term, first: $first, after: $after, filter: $filter) { nodes { ",
        issue_fields!(),
        " } pageInfo { hasNextPage endCursor } } }"
    ),
};

/// The issues of one thing that has some (a team, a cycle, a view, a project, a label), asked
/// for by its id: the id is a variable, and what the issues are of is the one thing that differs.
macro_rules! issues_of {
    ($name:literal, $root:literal) => {
        Operation {
            name: $name,
            text: concat!(
                "query ",
                $name,
                "($id: String!, $first: Int!, $after: String, $filter: IssueFilter) { viewer { ",
                viewer_fields!(),
                " } ",
                $root,
                "(id: $id) { issues(first: $first, after: $after, orderBy: updatedAt, filter: $filter) { \
                 nodes { ",
                issue_fields!(),
                " } pageInfo { hasNextPage endCursor } } } }"
            ),
        }
    };
}

/// The issues of a team.
pub const TEAM_ISSUES: Operation = issues_of!("TeamIssues", "team");
/// The issues of a cycle.
pub const CYCLE_ISSUES: Operation = issues_of!("CycleIssues", "cycle");
/// The issues a saved view of issues shows.
pub const VIEW_ISSUES: Operation = issues_of!("ViewIssues", "customView");
/// The issues of a project.
pub const PROJECT_ISSUES: Operation = issues_of!("ProjectIssues", "project");
/// The issues that have a label.
pub const LABEL_ISSUES: Operation = issues_of!("LabelIssues", "issueLabel");

/// The favorites of the person: what Linear starred (views, projects, cycles, labels, and what is not
/// a list here).
pub const LIST_FAVORITES: Operation = Operation {
    name: "ListFavorites",
    text: "query ListFavorites($first: Int!, $after: String) { \
           favorites(first: $first, after: $after) { nodes { type sortOrder \
           customView { id name modelName team { key } } project { id name } \
           cycle { id number name endsAt team { key } } label { id name team { key } } } \
           pageInfo { hasNextPage endCursor } } }",
};

/// The teams the person is a member of (the active memberships), in the order of their own list of
/// teams, each with the cycle that runs now.
pub const LIST_TEAMS: Operation = Operation {
    name: "ListTeams",
    text: "query ListTeams($first: Int!, $after: String) { \
           viewer { teamMemberships(first: $first, after: $after, includeArchived: false) { \
           nodes { sortOrder team { id key name activeCycle { id number name endsAt } } } \
           pageInfo { hasNextPage endCursor } } } }",
};

/// The saved views (of issues and of other things: the program keeps those of issues).
pub const LIST_VIEWS: Operation = Operation {
    name: "ListViews",
    text: "query ListViews($first: Int!, $after: String) { \
           customViews(first: $first, after: $after) { nodes { id name modelName team { key } } \
           pageInfo { hasNextPage endCursor } } }",
};

/// The projects that are not finished.
pub const LIST_PROJECTS: Operation = Operation {
    name: "ListProjects",
    text: "query ListProjects($first: Int!, $after: String, $filter: ProjectFilter) { \
           projects(first: $first, after: $after, filter: $filter) { nodes { id name } \
           pageInfo { hasNextPage endCursor } } }",
};

/// What the filters of a list offer to choose from: the states (by name), the people and the labels.
/// The projects are the ones of the lists a person can go to.
pub const FILTER_OPTIONS: Operation = Operation {
    name: "FilterOptions",
    text: "query FilterOptions { workflowStates(first: 250) { nodes { name type position } } \
           users(first: 250) { nodes { id displayName isMe } } \
           issueLabels(first: 250) { nodes { name color } } }",
};

/// A comment: its id, its words, when it was written and edited, and who wrote it (and whether that is
/// the person, who alone may edit it).
macro_rules! comment_fields {
    () => {
        "id body createdAt editedAt user { displayName isMe }"
    };
}

/// An issue another issue is tied to, as much as names it and opens it.
macro_rules! neighbor_fields {
    () => {
        "id identifier title state { id name type position }"
    };
}

/// One issue with what a row does not carry: its description, its first comments, and the issues
/// around it (its parent, its parts and what it is tied to, the first fifty of each; read only).
pub const ISSUE: Operation = Operation {
    name: "Issue",
    text: concat!(
        "query Issue($id: String!) { issue(id: $id) { ",
        issue_fields!(),
        " description parent { ",
        neighbor_fields!(),
        " } children(first: 50) { nodes { ",
        neighbor_fields!(),
        " } } relations(first: 50) { nodes { type relatedIssue { ",
        neighbor_fields!(),
        " } } } inverseRelations(first: 50) { nodes { type issue { ",
        neighbor_fields!(),
        " } } } comments(first: 50) { nodes { ",
        comment_fields!(),
        " } pageInfo { hasNextPage endCursor } } \
         attachments(first: 20, filter: { url: { startsWith: \"https://github.com/\" } }) { \
         nodes { title subtitle url } } } }"
    ),
};

/// The person's inbox: the first notifications Linear's Inbox shows them, what each is about, and the
/// issue of each that is about one (as a row of a list has it).
/// Whether the person's inbox has an unread notification of an issue: the first of the unread ones,
/// each only as far as the issue it is of. Asked with each refresh of everything, for the mark of the
/// inbox in the head.
pub const INBOX_UNREAD: Operation = Operation {
    name: "InboxUnread",
    text: "query InboxUnread { inboxNotifications(first: 50, unreadOnly: true) { nodes { \
           ... on IssueNotification { issue { id } } } } }",
};

pub const INBOX: Operation = Operation {
    name: "Inbox",
    text: concat!(
        "query Inbox($first: Int!) { viewer { ",
        viewer_fields!(),
        " } inboxNotifications(first: $first) { nodes { type category readAt createdAt \
         actor { displayName } botActor { name } externalUserActor { name } \
         ... on IssueNotification { issue { ",
        issue_fields!(),
        " } } } pageInfo { hasNextPage } } }"
    ),
};

/// The projects of the workspace, the first that changed last: each as its line in the list has it
/// (its state, health, progress, lead, dates and the summary of its last update). Their milestones and
/// the words of an update are asked for one project at a time (`PROJECT`).
pub const PROJECTS: Operation = Operation {
    name: "Projects",
    text: concat!(
        "query Projects($first: Int!) { viewer { ",
        viewer_fields!(),
        " } projects(first: $first, orderBy: updatedAt) { nodes { id name url progress health \
         startDate startDateResolution targetDate targetDateResolution \
         status { id name type position } lead { displayName isMe } \
         lastUpdate { createdAt shortSummary isStale user { displayName } } } \
         pageInfo { hasNextPage } } }"
    ),
};

/// What one project carries that its line does not: the words of its last update and its first
/// milestones.
pub const PROJECT: Operation = Operation {
    name: "Project",
    text: "query Project($id: String!) { project(id: $id) { id lastUpdate { body } \
           projectMilestones(first: 20) { nodes { name progress targetDate } } } }",
};

/// Marks every notification of one issue read in the person's inbox (`input: { issueId }`), at
/// `readAt`: what Linear's Inbox does when the person reads them there. Nothing else of a
/// notification is written (no archive, no snooze).
pub const MARK_READ: Operation = Operation {
    name: "NotificationMarkReadAll",
    text: "mutation NotificationMarkReadAll($input: NotificationEntityInput!, $readAt: DateTime!) { \
           notificationMarkReadAll(input: $input, readAt: $readAt) { success } }",
};

/// The teams an issue can be made in.
pub const NEW_ISSUE_TEAMS: Operation = Operation {
    name: "NewIssueTeams",
    text: "query NewIssueTeams { teams(first: 50) { nodes { id key name } } }",
};

/// What a new issue of one team can be given: its states, labels, projects and members, and the
/// labels of the whole workspace (they belong to no team); and what an issue of it can be moved to:
/// its cycles (the one running, and the upcoming ones: Linear gives the newest first, so as many are
/// asked for as it makes ahead) and the scale it estimates by.
pub const NEW_ISSUE_OPTIONS: Operation = Operation {
    name: "NewIssueOptions",
    text: "query NewIssueOptions($team: String!) { team(id: $team) { id defaultIssueState { id } \
         states(first: 50) { nodes { id name type position } } \
         labels(first: 100) { nodes { id name color isGroup } } \
         projects(first: 50) { nodes { id name completedAt canceledAt } } \
         members(first: 100) { nodes { id displayName isMe active } } \
         cyclesEnabled issueEstimationType issueEstimationAllowZero issueEstimationExtended \
         activeCycle { id number name } \
         cycles(first: 50, filter: { isFuture: { eq: true } }) { nodes { id number name } } } \
         issueLabels(first: 100, filter: { team: { null: true } }) { nodes { id name color isGroup } } }",
};

/// One of the four things the pen writes: the issue the person made in the form. Sent by
/// `linear::create` and by nothing else (it is called from the session and from the command line).
pub const ISSUE_CREATE: Operation = Operation {
    name: "IssueCreate",
    text: "mutation IssueCreate($input: IssueCreateInput!) { issueCreate(input: $input) { \
           success issue { id identifier url } } }",
};

/// The other thing the pen writes: the change the person made to one issue or to several (one
/// operation for both, so there is one way to change). What it may carry is `Change`'s and nothing
/// else (`change.rs`). Sent by `linear::change` and `linear::change_all`, which share one function that
/// sends it.
pub const ISSUE_BATCH_UPDATE: Operation = Operation {
    name: "IssueBatchUpdate",
    text: concat!(
        "mutation IssueBatchUpdate($ids: [UUID!]!, $input: IssueUpdateInput!) { \
         issueBatchUpdate(ids: $ids, input: $input) { success issues { ",
        issue_fields!(),
        " } } }"
    ),
};

/// The third thing the pen writes: a comment on an issue, in the words the person wrote (and nothing
/// else rides with it). Sent by `linear::comment` and by nothing else.
pub const COMMENT_CREATE: Operation = Operation {
    name: "CommentCreate",
    text: concat!(
        "mutation CommentCreate($input: CommentCreateInput!) { commentCreate(input: $input) { \
         success comment { ",
        comment_fields!(),
        " } } }"
    ),
};

/// One comment, by Linear's id: its words whole (the issue's own question cuts none, but tells only the
/// first fifty), whose it is, and the issue it is on. What an edit begins from and is checked against.
pub const COMMENT: Operation = Operation {
    name: "Comment",
    text: concat!(
        "query Comment($id: String!) { comment(id: $id) { ",
        comment_fields!(),
        " issue { id identifier } } }"
    ),
};

/// The words of a comment of the person's, as they edited them: the body and nothing else, to the one
/// comment named. Sent by `linear::edit` and by nothing else; the program sends it only for a comment
/// Linear says is the person's.
pub const COMMENT_UPDATE: Operation = Operation {
    name: "CommentUpdate",
    text: concat!(
        "mutation CommentUpdate($id: String!, $input: CommentUpdateInput!) { \
         commentUpdate(id: $id, input: $input) { success comment { ",
        comment_fields!(),
        " } } }"
    ),
};

/// The fourth: where Linear takes a picture the person adds (to a comment, a new issue or the end of
/// a description). It makes nothing of the person's: it gives an address to send the file to and the
/// address the file will have. Sent by `linear::upload` and by nothing else.
pub const FILE_UPLOAD: Operation = Operation {
    name: "FileUpload",
    text: "mutation FileUpload($contentType: String!, $filename: String!, $size: Int!) { \
           fileUpload(contentType: $contentType, filename: $filename, size: $size) { \
           success uploadFile { uploadUrl assetUrl headers { key value } } } }",
};

/// Every operation there is, for the test that scans for a `mutation`.
#[cfg(test)]
pub const ALL: [Operation; 28] = [
    VERIFY,
    MY_ISSUES,
    ALL_ISSUES,
    SEARCH_ISSUES,
    INBOX,
    INBOX_UNREAD,
    PROJECTS,
    PROJECT,
    TEAM_ISSUES,
    CYCLE_ISSUES,
    VIEW_ISSUES,
    PROJECT_ISSUES,
    LABEL_ISSUES,
    LIST_FAVORITES,
    LIST_TEAMS,
    LIST_VIEWS,
    LIST_PROJECTS,
    FILTER_OPTIONS,
    ISSUE,
    NEW_ISSUE_TEAMS,
    NEW_ISSUE_OPTIONS,
    ISSUE_CREATE,
    ISSUE_BATCH_UPDATE,
    COMMENT_CREATE,
    COMMENT,
    COMMENT_UPDATE,
    FILE_UPLOAD,
    MARK_READ,
];

/// The variables of the question for one issue: Linear's id of it, or its identifier.
#[derive(Debug, Serialize)]
pub struct Lookup<'a> {
    pub id: &'a str,
}

/// The variables of the question for the choices of one team.
#[derive(Debug, Serialize)]
pub struct Team<'a> {
    pub team: &'a str,
}

/// The variables of `IssueCreate`.
#[derive(Debug, Serialize)]
pub struct Create<'a> {
    pub input: &'a super::NewIssue,
}

/// The variables of `CommentCreate`.
#[derive(Debug, Serialize)]
pub struct Commented<'a> {
    pub input: super::comment::Input<'a>,
}

/// The variables of `IssueBatchUpdate`: the issues by Linear's id, and what is changed in them (a
/// `Change`'s input, or the one description the pictures are added to: `describe.rs`).
#[derive(Debug, Serialize)]
pub struct Batch<'a, I: Serialize> {
    pub ids: &'a [String],
    pub input: I,
}

/// The variables of `FileUpload`: what the file is, and how large.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Upload<'a> {
    pub content_type: &'a str,
    pub filename: &'a str,
    pub size: u64,
}

/// The variables of a page of issues: the filter is Linear's `IssueFilter` (`Filters::to_value`),
/// `null` where there is none.
#[derive(Debug, Serialize)]
pub struct Page<'a> {
    pub first: u32,
    pub after: Option<&'a str>,
    pub filter: serde_json::Value,
}

/// The variables of a page of the issues a search finds: the words, and the page.
#[derive(Debug, Serialize)]
pub struct Searched<'a> {
    pub term: &'a str,
    #[serde(flatten)]
    pub page: Page<'a>,
}

/// The variables of a page of the issues of one thing that has some: its id, and the page.
#[derive(Debug, Serialize)]
pub struct Owned<'a> {
    pub id: &'a str,
    #[serde(flatten)]
    pub page: Page<'a>,
}

/// The variables of a page of names (of favorites, teams, views), and of the projects that are
/// not finished.
#[derive(Debug, Serialize)]
pub struct Named<'a> {
    pub first: u32,
    pub after: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The guard: each operation is a plain query, or is a mutation of the list; and it is named by
    /// its text.
    #[test]
    fn every_operation_reads_or_is_the_one_listed_write() {
        let mut writes = Vec::new();
        for operation in ALL {
            assert!(operation.allowed(), "{}", operation.name);
            if operation.reads_only() {
                let lower = operation.text.to_ascii_lowercase();
                assert!(!lower.contains("mutation"), "{}", operation.name);
                assert!(!lower.contains("subscription"), "{}", operation.name);
                assert!(
                    operation
                        .text
                        .starts_with(&format!("query {}", operation.name)),
                    "{} is named by its text",
                    operation.name
                );
            } else {
                assert!(operation.is_listed_write(), "{}", operation.name);
                assert!(
                    operation
                        .text
                        .starts_with(&format!("mutation {}(", operation.name)),
                    "{} is named by its text",
                    operation.name
                );
                writes.push(operation.name);
            }
        }
        // The pen writes five things and asks where to put a picture, and the list says which.
        let listed = [
            "IssueCreate",
            "IssueBatchUpdate",
            "CommentCreate",
            "CommentUpdate",
            "FileUpload",
            "NotificationMarkReadAll",
        ];
        assert_eq!(ALLOWED_MUTATIONS, listed);
        assert_eq!(writes, listed);
    }

    /// A query added and left out of `ALL` would not be scanned: every `Operation` of this file is in it.
    #[test]
    fn all_lists_every_query_of_the_file() {
        let source = include_str!("query.rs");
        let declared = source
            .lines()
            .filter(|line| {
                line.starts_with("pub const ")
                    && (line.ends_with(": Operation = Operation {")
                        || line.contains("= issues_of!("))
            })
            .count();
        assert_eq!(declared, ALL.len());
    }

    #[test]
    fn a_mutation_that_is_not_listed_is_not_allowed_however_it_is_dressed() {
        let not_allowed = |name, text| {
            let operation = Operation { name, text };
            assert!(!operation.allowed(), "{text}");
        };
        not_allowed("Bad", "mutation Bad { issueDelete(id: \"x\") { success } }");
        // Named as the listed one, and another mutation.
        not_allowed(
            "IssueCreate",
            "mutation IssueDelete($id: String!) { issueDelete(id: $id) { success } }",
        );
        // Text of the listed one, under another name.
        not_allowed("Other", ISSUE_CREATE.text);
        // A query with a mutation behind it, and the listed mutation with another behind it.
        not_allowed("Hidden", "query Hidden { viewer { id } } mutation M { x }");
        not_allowed(
            "IssueCreate",
            "mutation IssueCreate($input: IssueCreateInput!) { issueCreate(input: $input) { success } } \
             mutation IssueDelete { issueDelete(id: \"x\") { success } }",
        );
        not_allowed(
            "IssueCreate",
            "mutation IssueCreate { issueCreate { success } } query Q { viewer { id } }",
        );
        not_allowed("Live", "subscription Live { issueUpdated { id } }");
        // The listed one under the name of the other, and the two in one text.
        not_allowed("IssueCreate", ISSUE_BATCH_UPDATE.text);
        not_allowed("IssueBatchUpdate", ISSUE_CREATE.text);
        not_allowed(
            "IssueBatchUpdate",
            "mutation IssueBatchUpdate($ids: [UUID!]!, $input: IssueUpdateInput!) { \
             issueBatchUpdate(ids: $ids, input: $input) { success } } \
             mutation IssueArchive($id: String!) { issueArchive(id: $id) { success } }",
        );
        // The comment's text under another name, and under the name of another write.
        not_allowed("Other", COMMENT_CREATE.text);
        not_allowed("IssueCreate", COMMENT_CREATE.text);
        not_allowed("CommentCreate", ISSUE_BATCH_UPDATE.text);
        // The edit of a comment is listed, and is no other write; the way to take a comment away is not
        // listed, and does not ride with it.
        not_allowed("Other", COMMENT_UPDATE.text);
        not_allowed("CommentCreate", COMMENT_UPDATE.text);
        not_allowed("CommentUpdate", COMMENT_CREATE.text);
        not_allowed(
            "CommentUpdate",
            "mutation CommentUpdate($id: String!, $input: CommentUpdateInput!) { commentUpdate(id: $id, input: $input) { success } } \
             mutation CommentDelete($id: String!) { commentDelete(id: $id) { success } }",
        );
        assert!(COMMENT_UPDATE.allowed());
        assert!(!COMMENT_UPDATE.reads_only());
        assert!(COMMENT.reads_only());
        // The ways to take away what was said are not listed.
        not_allowed(
            "CommentCreate",
            "mutation CommentCreate($input: CommentCreateInput!) { commentCreate(input: $input) { success } } \
             mutation CommentDelete($id: String!) { commentDelete(id: $id) { success } }",
        );
        not_allowed(
            "CommentDelete",
            "mutation CommentDelete($id: String!) { commentDelete(id: $id) { success } }",
        );
        // The picture's place is listed; the ways to take a file away or to make another are not.
        not_allowed("Other", FILE_UPLOAD.text);
        not_allowed("FileUpload", COMMENT_CREATE.text);
        not_allowed(
            "FileUpload",
            "mutation FileUpload($id: String!) { fileUpload(id: $id) { success } } \
             mutation ImageUploadFromUrl($url: String!) { imageUploadFromUrl(url: $url) { success } }",
        );
        not_allowed(
            "ImageUploadFromUrl",
            "mutation ImageUploadFromUrl($url: String!) { imageUploadFromUrl(url: $url) { success } }",
        );
        not_allowed(
            "AttachmentDelete",
            "mutation AttachmentDelete($id: String!) { attachmentDelete(id: $id) { success } }",
        );
        // The ones listed are allowed and are not queries.
        assert!(FILE_UPLOAD.allowed());
        assert!(!FILE_UPLOAD.reads_only());
        assert!(COMMENT_CREATE.allowed());
        assert!(!COMMENT_CREATE.reads_only());
        assert!(ISSUE_CREATE.allowed());
        assert!(!ISSUE_CREATE.reads_only());
        assert!(ISSUE_BATCH_UPDATE.allowed());
        assert!(!ISSUE_BATCH_UPDATE.reads_only());
        assert!(VERIFY.allowed());
        assert!(!VERIFY.is_listed_write());
    }

    #[test]
    fn the_filter_is_the_open_issues() {
        let page = Page {
            first: 100,
            after: None,
            filter: crate::linear::Filters::default().to_value(crate::linear::States::Open),
        };
        assert_eq!(
            serde_json::to_string(&page).unwrap(),
            r#"{"first":100,"after":null,"filter":{"state":{"type":{"nin":["completed","canceled"]}}}}"#
        );
    }
}
