//! What is asked of Linear: every query of the program, as constant text. The pen only reads, so
//! none of them is a `mutation`: `ALL` lists them for the test that scans for one, and the client
//! refuses to send what is not a plain `query` (`mod.rs`). Nothing from outside is put into the
//! text: what varies goes in the `variables` of the request.

use serde::Serialize;

/// One query: its name (which the stand-in `curl` of the tests tells them apart by) and its text.
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
}

/// The issue fields of a row of the list. The description and the comments wait for the issue to be
/// opened.
macro_rules! issue_fields {
    () => {
        "id identifier title priority updatedAt url branchName \
         state { id name type position } \
         labels(first: 10) { nodes { name color } } \
         team { key } cycle { number } project { name } \
         assignee { displayName isMe }"
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

/// The open issues assigned to the person.
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

/// The open issues of the whole workspace.
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

/// Every query there is, for the test that scans for a `mutation`.
#[cfg(test)]
pub const ALL: [Operation; 3] = [VERIFY, MY_ISSUES, ALL_ISSUES];

/// The variables of a page of issues.
#[derive(Debug, Serialize)]
pub struct Page<'a> {
    pub first: u32,
    pub after: Option<&'a str>,
    pub filter: Filter,
}

/// The issues that are still open: not completed and not canceled.
#[derive(Debug, Serialize)]
pub struct Filter {
    pub state: StateFilter,
}

#[derive(Debug, Serialize)]
pub struct StateFilter {
    #[serde(rename = "type")]
    pub kind: TypeFilter,
}

#[derive(Debug, Serialize)]
pub struct TypeFilter {
    pub nin: [&'static str; 2],
}

impl Filter {
    #[must_use]
    pub fn open() -> Self {
        Self {
            state: StateFilter {
                kind: TypeFilter {
                    nin: ["completed", "canceled"],
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pen reads. This is the guard: no query of the program writes, and each is one plain query.
    #[test]
    fn no_query_writes() {
        for operation in ALL {
            let lower = operation.text.to_ascii_lowercase();
            assert!(!lower.contains("mutation"), "{}", operation.name);
            assert!(!lower.contains("subscription"), "{}", operation.name);
            assert!(operation.reads_only(), "{}", operation.name);
            assert!(
                operation
                    .text
                    .starts_with(&format!("query {}", operation.name)),
                "{} is named by its text",
                operation.name
            );
        }
    }

    /// A query added and left out of `ALL` would not be scanned: every `Operation` of this file is in it.
    #[test]
    fn all_lists_every_query_of_the_file() {
        let source = include_str!("query.rs");
        let declared = source
            .lines()
            .filter(|line| {
                line.starts_with("pub const ") && line.ends_with(": Operation = Operation {")
            })
            .count();
        assert_eq!(declared, ALL.len());
    }

    #[test]
    fn the_guard_sees_a_mutation() {
        let bad = Operation {
            name: "Bad",
            text: "mutation Bad { issueDelete(id: \"x\") { success } }",
        };
        assert!(!bad.reads_only());
        let hidden = Operation {
            name: "Hidden",
            text: "query Hidden { viewer { id } } mutation M { x }",
        };
        assert!(!hidden.reads_only());
    }

    #[test]
    fn the_filter_is_the_open_issues() {
        let page = Page {
            first: 100,
            after: None,
            filter: Filter::open(),
        };
        assert_eq!(
            serde_json::to_string(&page).unwrap(),
            r#"{"first":100,"after":null,"filter":{"state":{"type":{"nin":["completed","canceled"]}}}}"#
        );
    }
}
