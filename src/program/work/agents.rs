//! The agents the program can start: which of them are installed, and the command that starts one.
//! An agent is found in the `PATH` and is run with its prompt as its one argument, and with no flag
//! that lets it do what it would ask about.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Cursor,
}

impl Agent {
    /// In the order the page lists them.
    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Cursor];

    /// The name the page and the calls use.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
        }
    }

    /// What the person is told.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Cursor => "Cursor",
        }
    }

    /// The program that is run.
    #[must_use]
    pub fn program(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor-agent",
        }
    }

    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.id() == id)
    }

    /// The command that starts the agent on `prompt`: the program and its one argument.
    #[must_use]
    pub fn command(self, prompt: &str) -> Vec<String> {
        vec![self.program().to_owned(), prompt.to_owned()]
    }
}

/// Whether `program` is a file that can be run in one of the directories of the `PATH`.
#[must_use]
pub fn on_path(path: &[PathBuf], program: &str) -> bool {
    path.iter().any(|dir| runnable(&dir.join(program)))
}

fn runnable(file: &Path) -> bool {
    std::fs::metadata(file)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// The agents installed, in the order of `Agent::ALL`.
#[must_use]
pub fn installed(path: &[PathBuf]) -> Vec<Agent> {
    Agent::ALL
        .into_iter()
        .filter(|agent| on_path(path, agent.program()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn program(dir: &Path, name: &str, mode: u32) {
        let file = dir.join(name);
        fs::write(&file, "#!/bin/sh\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn the_agents_installed_are_the_ones_found_as_programs_in_the_path() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        program(first.path(), "cursor-agent", 0o755);
        program(second.path(), "claude", 0o755);
        // A file that cannot be run is not an agent.
        program(second.path(), "codex", 0o644);
        let path = vec![first.path().to_owned(), second.path().to_owned()];
        assert_eq!(installed(&path), vec![Agent::Claude, Agent::Cursor]);
        assert!(on_path(&path, "claude"));
        assert!(!on_path(&path, "codex"));
        assert!(installed(&[]).is_empty());
    }

    #[test]
    fn an_agent_is_started_with_its_prompt_and_nothing_else() {
        assert_eq!(
            Agent::Cursor.command("Work on it"),
            vec!["cursor-agent".to_owned(), "Work on it".to_owned()]
        );
        for agent in Agent::ALL {
            let words = agent.command("x");
            assert_eq!(words.len(), 2, "{words:?}");
            assert_eq!(Agent::parse(agent.id()), Some(agent));
        }
        assert_eq!(Agent::parse("gemini"), None);
    }
}
